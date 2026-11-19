//! Video library: folder scan, content id, upsert (SPEC-002).
//!
//! Identity rules:
//! - id = blake3(size_le ‖ first 64 KiB ‖ last 64 KiB), first 16 bytes as hex (32 chars).
//! - Same path + size + mtime as stored → not re-hashed (R5).
//! - Same id at a new path → rename/move: path updated, id and history kept (R4).
//!   Title follows the new file name only if it was never edited (R8).
//! - Same content in two paths → one record, last path in sorted walk order; duplicate logged (R4).
//! - Content changed in place (new id at a known path whose old id is gone) → the row takes the
//!   new id when it has no history; otherwise the old id is kept so history stays attached.
//! - Row not found → available = 0, row kept (R6).

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::now_ms;
use crate::error::{AppError, AppResult};

pub const EXTENSIONS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi"];
const CHUNK: u64 = 64 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub found: u32,
    pub added: u32,
    pub updated: u32,
    pub removed: u32,
    pub skipped: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LibraryChange {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
}

impl LibraryChange {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Playable {
    Unknown,
    Yes,
    No,
}

impl Playable {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Yes => "yes",
            Self::No => "no",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "yes" => Self::Yes,
            "no" => Self::No,
            _ => Self::Unknown,
        }
    }
}

/// Mirrors `Song` in ipc.ts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub path: String,
    pub file_name: String,
    pub title: String,
    pub artist: Option<String>,
    pub extension: String,
    pub size_bytes: i64,
    pub duration_ms: Option<i64>,
    pub playable: Playable,
    pub available: bool,
    /// "none" when there is no active melody reference.
    pub reference_status: String,
}

/// `update_song` patch. `artist: null` clears it; absent keys are left alone.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SongPatch {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub artist: Option<Option<String>>,
    pub duration_ms: Option<i64>,
    pub playable: Option<Playable>,
}

fn double_option<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

/// Scans `root` and syncs the `songs` table. Hashing happens without holding the DB lock;
/// callers must run this off the main thread and serialize scans (one at a time).
pub fn scan(
    db: &Mutex<Connection>,
    root: &Path,
    recursive: bool,
) -> AppResult<(ScanSummary, LibraryChange)> {
    let started = Instant::now();
    if !root.is_dir() {
        return Err(AppError::new(
            "library_folder_not_found",
            format!(
                "A pasta {} não foi encontrada ou não está acessível.",
                root.display()
            ),
        ));
    }
    let known: HashMap<String, (String, i64, i64)> = {
        let conn = lock(db);
        let mut stmt = conn.prepare("SELECT path, id, size_bytes, modified_ms FROM songs")?;
        stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))?
            .collect::<Result<_, _>>()?
    };

    let mut summary = ScanSummary::default();
    let mut files = Vec::new();
    walk(root, recursive, &mut files, &mut summary.skipped);
    files.sort();

    // ponytail: sequential hashing (128 KiB read per new/changed file); add a small thread pool
    // if first scans of large libraries on HDD prove slow.
    let mut found = Vec::new();
    for path in files {
        match inspect(&path, &known) {
            Ok(Some(f)) => found.push(f),
            Ok(None) => summary.skipped += 1,
            Err(e) => {
                log::warn!("skipping unreadable file {}: {e}", path.display());
                summary.skipped += 1;
            }
        }
    }
    summary.found = found.len() as u32;

    let mut conn = lock(db);
    let change = apply(&mut conn, found)?;
    summary.added = change.added.len() as u32;
    summary.updated = change.updated.len() as u32;
    summary.removed = change.removed.len() as u32;
    summary.duration_ms = started.elapsed().as_millis() as u64;
    log::info!("library scan of {}: {summary:?}", root.display());
    Ok((summary, change))
}

pub fn lock(db: &Mutex<Connection>) -> std::sync::MutexGuard<'_, Connection> {
    db.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn walk(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>, skipped: &mut u32) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("cannot read folder {}: {e}", dir.display());
            return;
        }
    };
    for entry in entries.flatten() {
        // file_type() does not follow symlinks: no loops, no escaping the library folder.
        let Ok(ft) = entry.file_type() else { continue };
        let path = entry.path();
        if ft.is_dir() {
            if recursive {
                walk(&path, recursive, out, skipped);
            }
        } else if ft.is_file() {
            if is_supported(&path) {
                out.push(path);
            } else {
                *skipped += 1;
            }
        }
    }
}

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

struct Found {
    id: String,
    path: String,
    file_name: String,
    extension: String,
    size: i64,
    mtime: i64,
}

/// `Ok(None)` = path is not valid UTF-8 (cannot be stored/served safely).
fn inspect(
    path: &Path,
    known: &HashMap<String, (String, i64, i64)>,
) -> std::io::Result<Option<Found>> {
    let (Some(path_str), Some(file_name), Some(ext)) = (
        path.to_str(),
        path.file_name().and_then(|n| n.to_str()),
        path.extension().and_then(|e| e.to_str()),
    ) else {
        return Ok(None);
    };
    let meta = std::fs::metadata(path)?;
    let size = meta.len() as i64;
    let mtime = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let id = match known.get(path_str) {
        Some((id, s, m)) if *s == size && *m == mtime => id.clone(),
        _ => content_id(path)?,
    };
    Ok(Some(Found {
        id,
        path: path_str.to_owned(),
        file_name: file_name.to_owned(),
        extension: ext.to_ascii_lowercase(),
        size,
        mtime,
    }))
}

pub fn content_id(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    let mut buf = Vec::with_capacity(CHUNK as usize);
    (&mut file).take(CHUNK).read_to_end(&mut buf)?;
    hasher.update(&buf);
    buf.clear();
    file.seek(SeekFrom::Start(size.saturating_sub(CHUNK)))?;
    file.take(CHUNK).read_to_end(&mut buf)?;
    hasher.update(&buf);
    Ok(hasher.finalize().to_hex()[..32].to_owned())
}

fn stem(file_name: &str) -> &str {
    Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name)
}

struct Row {
    path: String,
    file_name: String,
    title: String,
    size: i64,
    mtime: i64,
    available: bool,
}

fn apply(conn: &mut Connection, found: Vec<Found>) -> AppResult<LibraryChange> {
    let tx = conn.transaction()?;
    let now = now_ms();

    // One record per content id: last path in sorted order wins.
    let mut by_id: HashMap<String, Found> = HashMap::new();
    for f in found {
        if let Some(prev) = by_id.get(&f.id) {
            log::info!(
                "duplicate content: {} and {} (keeping the latter)",
                prev.path,
                f.path
            );
        }
        by_id.insert(f.id.clone(), f);
    }

    let load = |tx: &rusqlite::Transaction| -> AppResult<HashMap<String, Row>> {
        let mut stmt = tx.prepare(
            "SELECT id, path, file_name, title, size_bytes, modified_ms, available FROM songs",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    Row {
                        path: r.get(1)?,
                        file_name: r.get(2)?,
                        title: r.get(3)?,
                        size: r.get(4)?,
                        mtime: r.get(5)?,
                        available: r.get(6)?,
                    },
                ))
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    };

    // Content changed in place: re-key the row when it has no history, else keep the old id.
    let existing = load(&tx)?;
    let path_owner: HashMap<&str, &str> = existing
        .iter()
        .map(|(id, r)| (r.path.as_str(), id.as_str()))
        .collect();
    let mut rekeys = Vec::new();
    for f in by_id.values() {
        if let Some(&old) = path_owner.get(f.path.as_str())
            && old != f.id
            && !existing.contains_key(&f.id)
            && !by_id.contains_key(old)
        {
            rekeys.push((old.to_owned(), f.id.clone()));
        }
    }
    for (old, new) in rekeys {
        match tx.execute("UPDATE songs SET id = ?1 WHERE id = ?2", (&new, &old)) {
            Ok(_) => log::info!("content changed in place: {old} -> {new}"),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                log::info!("content changed in place for {old}; keeping id because it has history");
                if let Some(mut f) = by_id.remove(&new) {
                    f.id = old.clone();
                    by_id.insert(old, f);
                }
            }
            Err(e) => return Err(e.into()),
        }
    }

    let existing = load(&tx)?;
    let found_paths: HashSet<&str> = by_id.values().map(|f| f.path.as_str()).collect();
    // Free every path that is about to be taken by another row (UNIQUE(path)), e.g. swaps
    // or a vanished file whose path now holds other content. Vanished rows keep `path#id`.
    for (id, row) in &existing {
        let moving = by_id.get(id).is_some_and(|f| f.path != row.path);
        let displaced = !by_id.contains_key(id) && found_paths.contains(row.path.as_str());
        if moving || displaced {
            tx.execute(
                "UPDATE songs SET path = path || '#' || id WHERE id = ?1",
                [id],
            )?;
        }
    }

    let mut change = LibraryChange::default();
    let mut ids: Vec<&String> = by_id.keys().collect();
    ids.sort();
    for id in ids {
        let f = &by_id[id];
        match existing.get(id) {
            None => {
                tx.execute(
                    "INSERT INTO songs (id, path, file_name, title, extension, size_bytes, modified_ms, added_ms, updated_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                    (id, &f.path, &f.file_name, stem(&f.file_name), &f.extension, f.size, f.mtime, now),
                )?;
                change.added.push(id.clone());
            }
            Some(row) => {
                if row.path == f.path && row.size == f.size && row.mtime == f.mtime && row.available
                {
                    continue;
                }
                let title = if row.title == stem(&row.file_name) {
                    stem(&f.file_name)
                } else {
                    row.title.as_str()
                };
                tx.execute(
                    "UPDATE songs SET path = ?2, file_name = ?3, title = ?4, extension = ?5, size_bytes = ?6,
                     modified_ms = ?7, available = 1, updated_ms = ?8 WHERE id = ?1",
                    (id, &f.path, &f.file_name, title, &f.extension, f.size, f.mtime, now),
                )?;
                change.updated.push(id.clone());
            }
        }
    }
    let mut gone: Vec<&String> = existing
        .iter()
        .filter(|(id, r)| r.available && !by_id.contains_key(*id))
        .map(|(id, _)| id)
        .collect();
    gone.sort();
    for id in gone {
        tx.execute(
            "UPDATE songs SET available = 0, updated_ms = ?2 WHERE id = ?1",
            (id, now),
        )?;
        change.removed.push(id.clone());
    }
    tx.commit()?;
    Ok(change)
}

const SONG_SELECT: &str =
    "SELECT s.id, s.path, s.file_name, s.title, s.artist, s.extension, s.size_bytes,
    s.duration_ms, s.playable, s.available, COALESCE(r.status, 'none')
    FROM songs s LEFT JOIN melody_references r ON r.song_id = s.id AND r.active = 1";

fn song_row(r: &rusqlite::Row) -> rusqlite::Result<Song> {
    Ok(Song {
        id: r.get(0)?,
        path: r.get(1)?,
        file_name: r.get(2)?,
        title: r.get(3)?,
        artist: r.get(4)?,
        extension: r.get(5)?,
        size_bytes: r.get(6)?,
        duration_ms: r.get(7)?,
        playable: Playable::parse(&r.get::<_, String>(8)?),
        available: r.get(9)?,
        reference_status: r.get(10)?,
    })
}

/// Available songs only (removed files are hidden, R6).
pub fn list_songs(conn: &Connection) -> AppResult<Vec<Song>> {
    let mut stmt = conn.prepare(&format!(
        "{SONG_SELECT} WHERE s.available = 1 ORDER BY s.title COLLATE NOCASE, s.id"
    ))?;
    let songs = stmt.query_map([], song_row)?.collect::<Result<_, _>>()?;
    Ok(songs)
}

pub fn get_song(conn: &Connection, id: &str) -> AppResult<Song> {
    conn.query_row(&format!("{SONG_SELECT} WHERE s.id = ?1"), [id], song_row)
        .optional()?
        .ok_or_else(|| AppError::new("not_found", "Música não encontrada."))
}

pub fn update_song(conn: &Connection, id: &str, patch: SongPatch) -> AppResult<Song> {
    let current = get_song(conn, id)?;
    let title = match patch.title {
        Some(t) => {
            let t = t.trim();
            if t.is_empty() || t.chars().count() > 300 {
                return Err(AppError::invalid_input(
                    "O título deve ter entre 1 e 300 caracteres.",
                ));
            }
            t.to_owned()
        }
        None => current.title,
    };
    let artist = match patch.artist {
        Some(a) => {
            let a = a.map(|a| a.trim().to_owned()).filter(|a| !a.is_empty());
            if a.as_ref().is_some_and(|a| a.chars().count() > 300) {
                return Err(AppError::invalid_input(
                    "O artista deve ter no máximo 300 caracteres.",
                ));
            }
            a
        }
        None => current.artist,
    };
    let duration = match patch.duration_ms {
        Some(d) if !(0..=24 * 3600 * 1000).contains(&d) => {
            return Err(AppError::invalid_input("Duração inválida."));
        }
        Some(d) => Some(d),
        None => current.duration_ms,
    };
    let playable = patch.playable.unwrap_or(current.playable);
    conn.execute(
        "UPDATE songs SET title = ?2, artist = ?3, duration_ms = ?4, playable = ?5, updated_ms = ?6 WHERE id = ?1",
        (id, title, artist, duration, playable.as_str(), now_ms()),
    )?;
    get_song(conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lib {
        dir: tempfile::TempDir,
        _db_dir: tempfile::TempDir,
        db: Mutex<Connection>,
    }

    fn lib() -> Lib {
        let db_dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&db_dir.path().join("t.db")).unwrap();
        Lib {
            dir: tempfile::tempdir().unwrap(),
            _db_dir: db_dir,
            db: Mutex::new(conn),
        }
    }

    impl Lib {
        fn write(&self, rel: &str, content: &[u8]) -> PathBuf {
            let p = self.dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, content).unwrap();
            p
        }
        fn scan(&self) -> (ScanSummary, LibraryChange) {
            scan(&self.db, self.dir.path(), true).unwrap()
        }
        fn songs(&self) -> Vec<Song> {
            list_songs(&lock(&self.db)).unwrap()
        }
    }

    #[test]
    fn indexes_supported_only() {
        let l = lib();
        l.write("a.mp4", b"aaa");
        l.write("b.MKV", b"bbb");
        l.write("sub/c.webm", b"ccc");
        l.write("notes.txt", b"x");
        let (s, c) = l.scan();
        assert_eq!((s.found, s.added, s.skipped), (3, 3, 1));
        assert_eq!(c.added.len(), 3);
        let songs = l.songs();
        assert_eq!(songs.len(), 3);
        let b = songs.iter().find(|s| s.file_name == "b.MKV").unwrap();
        assert_eq!((b.title.as_str(), b.extension.as_str()), ("b", "mkv"));
        assert_eq!(b.reference_status, "none");
        assert!(songs.iter().all(|s| s.id.len() == 32));

        // Non-recursive skips subfolders.
        let l2 = lib();
        l2.write("a.mp4", b"aaa");
        l2.write("sub/c.webm", b"ccc");
        let (s, _) = scan(&l2.db, l2.dir.path(), false).unwrap();
        assert_eq!(s.found, 1);
    }

    #[test]
    fn stable_id_on_rename() {
        let l = lib();
        let a = l.write("old.mp4", b"song-content");
        l.scan();
        let before = l.songs()[0].clone();
        std::fs::rename(&a, l.dir.path().join("new.mp4")).unwrap();
        let (s, c) = l.scan();
        assert_eq!((s.added, s.updated, s.removed), (0, 1, 0));
        assert_eq!(c.updated, vec![before.id.clone()]);
        let after = &l.songs()[0];
        assert_eq!(after.id, before.id);
        assert!(after.path.ends_with("new.mp4"));
        assert_eq!(after.title, "new", "unedited title follows the file name");

        // Edited title survives a rename.
        update_song(
            &lock(&l.db),
            &after.id,
            SongPatch {
                title: Some("Minha Música".into()),
                ..Default::default()
            },
        )
        .unwrap();
        std::fs::rename(l.dir.path().join("new.mp4"), l.dir.path().join("newer.mp4")).unwrap();
        l.scan();
        assert_eq!(l.songs()[0].title, "Minha Música");
    }

    #[test]
    fn swap_names_keeps_ids() {
        let l = lib();
        let a = l.write("a.mp4", b"AAAA");
        let b = l.write("b.mp4", b"BBBBB"); // different sizes: rename keeps mtime
        l.scan();
        let id_a = content_id(&a).unwrap();
        let tmp = l.dir.path().join("tmp.bin");
        std::fs::rename(&a, &tmp).unwrap();
        std::fs::rename(&b, &a).unwrap();
        std::fs::rename(&tmp, &b).unwrap();
        l.scan();
        let song_a = get_song(&lock(&l.db), &id_a).unwrap();
        assert!(song_a.path.ends_with("b.mp4"));
    }

    #[test]
    fn skips_unchanged() {
        let l = lib();
        l.write("a.mp4", b"aaa");
        l.scan();
        // Corrupt the stored id: an unchanged file must not be re-hashed, so it stays as is.
        lock(&l.db)
            .execute("UPDATE songs SET id = 'sentinel'", [])
            .unwrap();
        let (s, c) = l.scan();
        assert_eq!((s.found, s.added, s.updated, s.removed), (1, 0, 0, 0));
        assert!(c.is_empty());
        assert_eq!(l.songs()[0].id, "sentinel");
    }

    #[test]
    fn marks_removed() {
        let l = lib();
        let a = l.write("a.mp4", b"aaa");
        l.write("b.mp4", b"bbb");
        l.scan();
        let id_a = content_id(&a).unwrap();
        lock(&l.db)
            .execute(
                "INSERT INTO performances (song_id, algorithm_version, difficulty, competitive, score, metrics_json, capture_json, created_ms)
                 VALUES (?1, 1, 'normal', 0, 50, '{}', '{}', 1)",
                [&id_a],
            )
            .unwrap();
        std::fs::remove_file(&a).unwrap();
        let (s, c) = l.scan();
        assert_eq!(s.removed, 1);
        assert_eq!(c.removed, vec![id_a.clone()]);
        assert_eq!(l.songs().len(), 1);
        let conn = lock(&l.db);
        assert!(!get_song(&conn, &id_a).unwrap().available);
        let perf: i64 = conn
            .query_row("SELECT count(*) FROM performances", [], |r| r.get(0))
            .unwrap();
        assert_eq!(perf, 1, "history kept");
        drop(conn);

        // Comes back → available again, same id.
        l.write("a.mp4", b"aaa");
        let (s, _) = l.scan();
        assert_eq!(s.updated, 1);
        assert_eq!(l.songs().len(), 2);
    }

    #[test]
    fn duplicate_content_single_record() {
        let l = lib();
        l.write("x/a.mp4", b"same");
        l.write("y/b.mp4", b"same");
        let (s, _) = l.scan();
        assert_eq!((s.found, s.added), (2, 1));
        assert!(l.songs()[0].path.ends_with("b.mp4"));
    }

    #[test]
    fn content_changed_in_place() {
        let l = lib();
        let a = l.write("a.mp4", b"v1");
        l.scan();
        let v1 = l.songs()[0].id.clone();
        // No history → row takes the new id.
        std::fs::write(&a, b"version-2").unwrap();
        l.scan();
        let v2 = content_id(&a).unwrap();
        assert_eq!(l.songs()[0].id, v2);
        assert_ne!(v1, v2);
        // With history → old id kept.
        lock(&l.db)
            .execute(
                "INSERT INTO performances (song_id, algorithm_version, difficulty, competitive, score, metrics_json, capture_json, created_ms)
                 VALUES (?1, 1, 'normal', 0, 50, '{}', '{}', 1)",
                [&v2],
            )
            .unwrap();
        std::fs::write(&a, b"version-three").unwrap();
        l.scan();
        let songs = l.songs();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].id, v2);
        assert_eq!(songs[0].size_bytes, 13);
    }

    #[test]
    fn content_id_uses_head_and_tail() {
        let l = lib();
        let mut big = vec![0u8; 300 * 1024];
        let a = l.write("a.mp4", &big);
        let id0 = content_id(&a).unwrap();
        big[150 * 1024] = 1; // middle byte: not hashed
        std::fs::write(&a, &big).unwrap();
        assert_eq!(content_id(&a).unwrap(), id0);
        let last = big.len() - 1;
        big[last] = 1; // tail byte: hashed
        std::fs::write(&a, &big).unwrap();
        assert_ne!(content_id(&a).unwrap(), id0);
    }

    #[test]
    fn update_song_validates() {
        let l = lib();
        l.write("a.mp4", b"aaa");
        l.scan();
        let id = l.songs()[0].id.clone();
        let conn = lock(&l.db);
        let s = update_song(
            &conn,
            &id,
            serde_json::from_value(
                serde_json::json!({"artist": "  Fulano ", "durationMs": 1000, "playable": "yes"}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            (s.artist.as_deref(), s.duration_ms, s.playable),
            (Some("Fulano"), Some(1000), Playable::Yes)
        );
        let s = update_song(
            &conn,
            &id,
            serde_json::from_value(serde_json::json!({"artist": null})).unwrap(),
        )
        .unwrap();
        assert_eq!(s.artist, None);
        assert_eq!(
            update_song(
                &conn,
                &id,
                SongPatch {
                    title: Some("  ".into()),
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
            "invalid_input"
        );
        assert_eq!(
            update_song(&conn, "nope", SongPatch::default())
                .unwrap_err()
                .code,
            "not_found"
        );
    }
}
