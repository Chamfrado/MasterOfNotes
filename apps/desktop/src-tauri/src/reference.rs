//! Melody reference persistence and commands (SPEC-005, T-005-03).

use std::path::Path;

use rusqlite::{Connection, OptionalExtension};
use scoring_engine::{MidiTrackInfo, RefError};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::library::{self, lock};

/// Imported files are read fully into memory; real vocal lines are a few KB.
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_OFFSET_MS: i32 = 10_000;

/// Mirrors `RefNote` in ipc.ts (`lyric` always present, `null` when absent).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefNote {
    pub start_ms: u32,
    pub dur_ms: u32,
    pub midi: f32,
    pub lyric: Option<String>,
}

/// Mirrors `MelodyReference` in ipc.ts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub id: i64,
    pub song_id: String,
    pub source: String,
    pub status: String,
    pub revision: i64,
    pub offset_ms: i32,
    pub notes: Vec<RefNote>,
    /// Extracted references (SPEC-012 R3): vocals/mix level in dB measured when generated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice_level_db: Option<f32>,
    /// pt-BR, only on `import_reference` results (e.g. UltraStar duet). Omitted when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// Statuses the user can set (`processing` is internal).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserStatus {
    Experimental,
    Validated,
    Invalid,
}

impl UserStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Experimental => "experimental",
            Self::Validated => "validated",
            Self::Invalid => "invalid",
        }
    }
}

pub fn ref_err(e: RefError) -> AppError {
    let code = match e {
        RefError::TrackRequired => "midi_track_required",
        _ => "invalid_reference",
    };
    AppError::new(code, format!("Referência inválida: {e}."))
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<(Reference, String)> {
    Ok((
        Reference {
            id: r.get(0)?,
            song_id: r.get(1)?,
            source: r.get(2)?,
            status: r.get(3)?,
            revision: r.get(4)?,
            offset_ms: r.get(5)?,
            notes: Vec::new(),
            voice_level_db: r.get(7)?,
            notice: None,
        },
        r.get(6)?,
    ))
}

fn with_notes((mut reference, notes_json): (Reference, String)) -> AppResult<Reference> {
    reference.notes = parse_stored(&notes_json)?
        .notes
        .into_iter()
        .map(|n| RefNote {
            start_ms: n.start_ms,
            dur_ms: n.dur_ms,
            midi: n.midi,
            lyric: n.lyric,
        })
        .collect();
    Ok(reference)
}

/// Parses `notes_json` as written by `to_reference_json`.
pub fn parse_stored(notes_json: &str) -> AppResult<scoring_engine::MelodyReference> {
    scoring_engine::parse_reference_json(notes_json).map_err(|e| {
        log::error!("stored reference is invalid: {e}");
        AppError::new("internal", "A referência salva está corrompida.")
    })
}

const SELECT: &str = "SELECT id, song_id, source, status, revision, offset_ms, notes_json, voice_level_db FROM melody_references";

pub fn active(conn: &Connection, song_id: &str) -> AppResult<Option<Reference>> {
    conn.query_row(
        &format!("{SELECT} WHERE song_id = ?1 AND active = 1"),
        [song_id],
        row,
    )
    .optional()?
    .map(with_notes)
    .transpose()
}

pub fn get_active_by_id(conn: &Connection, id: i64) -> AppResult<Reference> {
    let (r, notes_json, active): (Reference, String, bool) = conn
        .query_row(
            "SELECT id, song_id, source, status, revision, offset_ms, notes_json, voice_level_db, active
             FROM melody_references WHERE id = ?1",
            [id],
            |r| row(r).and_then(|(x, n)| Ok((x, n, r.get(8)?))),
        )
        .optional()?
        .ok_or_else(|| AppError::new("not_found", "Referência não encontrada."))?;
    if !active {
        return Err(AppError::invalid_input(
            "Esta referência foi substituída por uma importação mais nova e não pode ser alterada.",
        ));
    }
    with_notes((r, notes_json))
}

/// Reads a user-chosen reference file after checking extension and size (Law 09).
fn read_file(file_path: &str, allowed: &[&str]) -> AppResult<(Vec<u8>, String)> {
    let path = Path::new(file_path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|e| allowed.contains(&e.as_str()))
        .ok_or_else(|| {
            AppError::invalid_input(format!(
                "Tipo de arquivo não suportado. Use: {}.",
                allowed.join(", ")
            ))
        })?;
    if !path.is_absolute() {
        return Err(AppError::invalid_input("Caminho de arquivo inválido."));
    }
    let meta = std::fs::metadata(path)
        .map_err(|_| AppError::new("not_found", format!("Arquivo não encontrado: {file_path}.")))?;
    if !meta.is_file() {
        return Err(AppError::invalid_input(
            "O caminho escolhido não é um arquivo.",
        ));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(AppError::invalid_input(
            "Arquivo grande demais para uma referência melódica (máximo 5 MB).",
        ));
    }
    // B4: the file may grow after the metadata check; never read more than the limit.
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(std::fs::File::open(path)?, MAX_FILE_BYTES + 1),
        &mut bytes,
    )?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(AppError::invalid_input(
            "Arquivo grande demais para uma referência melódica (máximo 5 MB).",
        ));
    }
    Ok((bytes, ext))
}

pub fn import(
    conn: &mut Connection,
    song_id: &str,
    file_path: &str,
    midi_track: Option<usize>,
) -> AppResult<Reference> {
    library::get_song(conn, song_id)?;
    let (bytes, ext) = read_file(file_path, &["mid", "midi", "json", "txt"])?;
    let mut notice = None;
    let (parsed, source) = match ext.as_str() {
        "json" => {
            let text = String::from_utf8(bytes).map_err(|_| {
                AppError::new("invalid_reference", "O arquivo JSON não está em UTF-8.")
            })?;
            (scoring_engine::parse_reference_json(&text), "json")
        }
        // UltraStar (SPEC-013): duets import only the first singer (R3).
        "txt" => {
            let (r, info) = scoring_engine::import_ultrastar(&bytes).map_err(ref_err)?;
            notice = info.duet.then(|| DUET_NOTICE.to_owned());
            (Ok(r), "ultrastar")
        }
        _ => (scoring_engine::import_midi(&bytes, midi_track), "midi"),
    };
    let reference = store(conn, song_id, source, &parsed.map_err(ref_err)?, None)?;
    Ok(Reference {
        notice,
        ..reference
    })
}

const DUET_NOTICE: &str = "Arquivo de dueto: importamos a parte do primeiro cantor.";

/// Detected notes become the new active reference: sung (T-005-07, `source = 'manual'`) or
/// extracted from the video (SPEC-012, `'extracted'`, with its `voice_level_db`). Same parser as an imported file.
pub fn store_notes(
    conn: &mut Connection,
    song_id: &str,
    source: &str,
    notes: Vec<scoring_engine::Note>,
    voice_level_db: Option<f32>,
) -> AppResult<Reference> {
    let json = scoring_engine::to_reference_json(&scoring_engine::MelodyReference {
        format_version: scoring_engine::REFERENCE_FORMAT_VERSION,
        notes,
    });
    let parsed = scoring_engine::parse_reference_json(&json).map_err(ref_err)?;
    store(conn, song_id, source, &parsed, voice_level_db)
}

/// Inserts `parsed` as the active experimental reference, deactivating the previous one.
fn store(
    conn: &mut Connection,
    song_id: &str,
    source: &str,
    parsed: &scoring_engine::MelodyReference,
    voice_level_db: Option<f32>,
) -> AppResult<Reference> {
    let now = now_ms();
    let tx = conn.transaction()?;
    // Previous active reference is kept for history (R5).
    tx.execute(
        "UPDATE melody_references SET active = 0, updated_ms = ?2 WHERE song_id = ?1 AND active = 1",
        (song_id, now),
    )?;
    tx.execute(
        "INSERT INTO melody_references (song_id, source, format_version, status, notes_json, created_ms, updated_ms, voice_level_db)
         VALUES (?1, ?2, ?3, 'experimental', ?4, ?5, ?5, ?6)",
        (
            song_id,
            source,
            parsed.format_version,
            scoring_engine::to_reference_json(parsed),
            now,
            voice_level_db,
        ),
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    log::info!(
        "reference {id} stored for song {song_id} from {source} ({} notes)",
        parsed.notes.len()
    );
    get_active_by_id(conn, id)
}

/// Offset change → revision + 1 and back to experimental (R4). Same offset → no-op.
pub fn set_offset(conn: &Connection, id: i64, offset_ms: i32) -> AppResult<Reference> {
    if !(-MAX_OFFSET_MS..=MAX_OFFSET_MS).contains(&offset_ms) {
        return Err(AppError::invalid_input(
            "O ajuste deve estar entre -10 e +10 segundos.",
        ));
    }
    let current = get_active_by_id(conn, id)?;
    if current.offset_ms == offset_ms {
        return Ok(current);
    }
    conn.execute(
        "UPDATE melody_references SET offset_ms = ?2, revision = revision + 1, status = 'experimental', updated_ms = ?3 WHERE id = ?1",
        (id, offset_ms, now_ms()),
    )?;
    get_active_by_id(conn, id)
}

pub fn set_status(conn: &Connection, id: i64, status: UserStatus) -> AppResult<Reference> {
    get_active_by_id(conn, id)?;
    conn.execute(
        "UPDATE melody_references SET status = ?2, updated_ms = ?3 WHERE id = ?1",
        (id, status.as_str(), now_ms()),
    )?;
    get_active_by_id(conn, id)
}

#[tauri::command]
pub async fn get_reference(
    state: State<'_, AppState>,
    song_id: String,
) -> AppResult<Option<Reference>> {
    active(&lock(state.db()?), &song_id)
}

#[tauri::command]
pub async fn list_midi_tracks(file_path: String) -> AppResult<Vec<MidiTrackInfo>> {
    let (bytes, _) = read_file(&file_path, &["mid", "midi"])?;
    scoring_engine::midi_tracks(&bytes).map_err(ref_err)
}

#[tauri::command]
pub async fn import_reference(
    state: State<'_, AppState>,
    song_id: String,
    file_path: String,
    midi_track: Option<usize>,
) -> AppResult<Reference> {
    import(&mut lock(state.db()?), &song_id, &file_path, midi_track)
}

#[tauri::command]
pub async fn set_reference_offset(
    state: State<'_, AppState>,
    reference_id: i64,
    offset_ms: i32,
) -> AppResult<Reference> {
    set_offset(&lock(state.db()?), reference_id, offset_ms)
}

#[tauri::command]
pub async fn set_reference_status(
    state: State<'_, AppState>,
    reference_id: i64,
    status: UserStatus,
) -> AppResult<Reference> {
    set_status(&lock(state.db()?), reference_id, status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const JSON: &str = r#"{"format":"mon-melody","version":1,"notes":[
        {"startMs":1000,"durMs":500,"midi":60,"lyric":"lá"},{"startMs":1600,"durMs":400,"midi":62}]}"#;

    struct Env {
        dir: tempfile::TempDir,
        conn: Connection,
        song: String,
    }

    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        conn.execute(
            "INSERT INTO songs (id, path, file_name, title, extension, size_bytes, modified_ms, added_ms, updated_ms)
             VALUES ('s1', 'C:/v/a.mp4', 'a.mp4', 'a', 'mp4', 1, 1, 1, 1)",
            [],
        )
        .unwrap();
        Env {
            dir,
            conn,
            song: "s1".into(),
        }
    }

    impl Env {
        fn file(&self, name: &str, content: &[u8]) -> String {
            let p: PathBuf = self.dir.path().join(name);
            std::fs::write(&p, content).unwrap();
            p.to_str().unwrap().to_owned()
        }
    }

    /// Format 0 MIDI, 480 tpq, default tempo (120 bpm): C4 for one beat at t=0, D4 at beat 2.
    fn midi() -> Vec<u8> {
        let track: &[u8] = &[
            0x00, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 0, // on, +480 off
            0x83, 0x60, 0x90, 62, 100, 0x83, 0x60, 0x80, 62, 0, // +480 on, +480 off
            0x00, 0xFF, 0x2F, 0x00,
        ];
        let mut v = b"MThd\0\0\0\x06\0\0\0\x01\x01\xE0MTrk".to_vec();
        v.extend_from_slice(&(track.len() as u32).to_be_bytes());
        v.extend_from_slice(track);
        v
    }

    #[test]
    fn import_json_and_reimport_keeps_history() {
        let mut e = env();
        let f = e.file("ref.JSON", JSON.as_bytes());
        let r1 = import(&mut e.conn, &e.song, &f, None).unwrap();
        assert_eq!(
            (r1.status.as_str(), r1.revision, r1.source.as_str()),
            ("experimental", 1, "json")
        );
        assert_eq!(r1.notes.len(), 2);
        assert_eq!(r1.notes[0].lyric.as_deref(), Some("lá"));
        assert_eq!(r1.notes[1].lyric, None);
        assert_eq!(active(&e.conn, &e.song).unwrap().unwrap().id, r1.id);

        let r2 = import(&mut e.conn, &e.song, &f, None).unwrap();
        assert_ne!(r1.id, r2.id);
        assert_eq!(active(&e.conn, &e.song).unwrap().unwrap().id, r2.id);
        let total: i64 = e
            .conn
            .query_row("SELECT count(*) FROM melody_references", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 2, "previous reference kept");
        // The replaced reference can no longer be edited.
        assert_eq!(
            set_offset(&e.conn, r1.id, 100).unwrap_err().code,
            "invalid_input"
        );
        // list_songs reports the active status.
        let songs = crate::library::list_songs(&e.conn).unwrap();
        assert_eq!(songs[0].reference_status, "experimental");
    }

    #[test]
    fn import_midi_file() {
        let mut e = env();
        let f = e.file("ref.mid", &midi());
        let tracks = scoring_engine::midi_tracks(&midi()).unwrap();
        assert_eq!(tracks.len(), 1);
        let r = import(&mut e.conn, &e.song, &f, None).unwrap();
        assert_eq!(r.source, "midi");
        assert_eq!(r.notes.len(), 2);
        assert_eq!((r.notes[1].start_ms, r.notes[1].midi), (1000, 62.0));
    }

    #[test]
    fn import_ultrastar_txt_with_duet_notice() {
        let mut e = env();
        // SPEC-013 criterion: #BPM:300, #GAP:1000, ": 0 4 0 Lá" → 1000 ms, 200 ms, MIDI 60.
        let solo = e.file(
            "song.txt",
            "#TITLE:Teste\n#BPM:300\n#GAP:1000\n: 0 4 0 Lá\nF 8 2 0 x\n: 12 4 2 la\nE\n".as_bytes(),
        );
        let r = import(&mut e.conn, &e.song, &solo, None).unwrap();
        assert_eq!(
            (r.source.as_str(), r.status.as_str()),
            ("ultrastar", "experimental")
        );
        assert_eq!(
            (r.notes[0].start_ms, r.notes[0].dur_ms, r.notes[0].midi),
            (1000, 200, 60.0)
        );
        assert_eq!(r.notes[0].lyric.as_deref(), Some("Lá"));
        assert_eq!(r.notes.len(), 2, "freestyle F left out");
        assert_eq!(r.notice, None);
        assert!(
            serde_json::to_value(&r).unwrap().get("notice").is_none(),
            "omitted, not null"
        );

        let duet = e.file(
            "duet.txt",
            b"#BPM:300\nP1\n: 0 4 0 a\n: 8 4 2 b\nP2\n: 0 4 7 c\nE\n",
        );
        let r = import(&mut e.conn, &e.song, &duet, None).unwrap();
        assert_eq!(r.notice.as_deref(), Some(DUET_NOTICE));
        assert_eq!(r.notes.len(), 2, "only P1");
        assert_eq!(
            serde_json::to_value(&r).unwrap()["notice"],
            "Arquivo de dueto: importamos a parte do primeiro cantor."
        );
        // Stored reference does not carry the notice.
        assert_eq!(active(&e.conn, &e.song).unwrap().unwrap().notice, None);

        // Bad line: pt-BR message with the line number (R4), nothing stored.
        let bad = e.file("bad.txt", b"#BPM:300\n: 0 4 x a\nE\n");
        let err = import(&mut e.conn, &e.song, &bad, None).unwrap_err();
        assert_eq!(err.code, "invalid_reference");
        assert!(err.message.contains("linha 2"), "{}", err.message);
        assert_eq!(active(&e.conn, &e.song).unwrap().unwrap().notice, None);
    }

    #[test]
    fn rejects_bad_files_and_saves_nothing() {
        let mut e = env();
        let wav = e.file("ref.wav", JSON.as_bytes());
        assert_eq!(
            import(&mut e.conn, &e.song, &wav, None).unwrap_err().code,
            "invalid_input"
        );
        // A .txt that is not UltraStar (no #BPM).
        let txt = e.file("ref.txt", JSON.as_bytes());
        assert_eq!(
            import(&mut e.conn, &e.song, &txt, None).unwrap_err().code,
            "invalid_reference"
        );
        let empty = e.file(
            "empty.json",
            br#"{"format":"mon-melody","version":1,"notes":[]}"#,
        );
        assert_eq!(
            import(&mut e.conn, &e.song, &empty, None).unwrap_err().code,
            "invalid_reference"
        );
        let big = e.file("big.json", &vec![b' '; (MAX_FILE_BYTES + 1) as usize]);
        assert_eq!(
            import(&mut e.conn, &e.song, &big, None).unwrap_err().code,
            "invalid_input"
        );
        let ok = e.file("ok.json", JSON.as_bytes());
        assert_eq!(
            import(&mut e.conn, "nope", &ok, None).unwrap_err().code,
            "not_found"
        );
        assert_eq!(
            import(&mut e.conn, &e.song, "relative.json", None)
                .unwrap_err()
                .code,
            "invalid_input"
        );
        let n: i64 = e
            .conn
            .query_row("SELECT count(*) FROM melody_references", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn offset_bumps_revision_and_resets_status() {
        let mut e = env();
        let f = e.file("ref.json", JSON.as_bytes());
        let r = import(&mut e.conn, &e.song, &f, None).unwrap();
        let v = set_status(&e.conn, r.id, UserStatus::Validated).unwrap();
        assert_eq!((v.status.as_str(), v.revision), ("validated", 1));
        let o = set_offset(&e.conn, r.id, -250).unwrap();
        assert_eq!(
            (o.status.as_str(), o.revision, o.offset_ms),
            ("experimental", 2, -250)
        );
        let same = set_offset(&e.conn, r.id, -250).unwrap();
        assert_eq!(same.revision, 2, "same offset is a no-op");
        assert_eq!(
            set_offset(&e.conn, r.id, 10_001).unwrap_err().code,
            "invalid_input"
        );
        assert_eq!(
            set_status(&e.conn, 999, UserStatus::Invalid)
                .unwrap_err()
                .code,
            "not_found"
        );
        assert!(serde_json::from_str::<UserStatus>("\"processing\"").is_err());
    }
}
