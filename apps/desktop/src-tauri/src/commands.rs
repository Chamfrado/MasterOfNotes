//! Tauri commands for settings and library. Names match `apps/desktop/src/lib/ipc.ts` 1:1.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_full::notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use rusqlite::Connection;
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::error::{AppError, AppResult};
use crate::library::{self, ScanSummary, Song, SongPatch, lock};
use crate::settings::{self, AppSettings};

type Watcher = Debouncer<notify_debouncer_full::notify::RecommendedWatcher, RecommendedCache>;

pub struct AppState {
    /// Err when the database could not be opened/migrated (SPEC-001 R4): every command
    /// returns that error so the UI can show the file path. Nothing is deleted.
    db: Result<Mutex<Connection>, AppError>,
    /// One scan at a time (watcher, rescan and folder change can overlap).
    scan_lock: Mutex<()>,
    watcher: Mutex<Option<Watcher>>,
}

impl AppState {
    pub fn new(db: AppResult<Connection>) -> Self {
        Self {
            db: db.map(Mutex::new),
            scan_lock: Mutex::new(()),
            watcher: Mutex::new(None),
        }
    }

    pub fn db(&self) -> AppResult<&Mutex<Connection>> {
        self.db.as_ref().map_err(Clone::clone)
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    settings::load(&lock(state.db()?))
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    patch: Map<String, Value>,
) -> AppResult<AppSettings> {
    settings::update(&mut lock(state.db()?), patch)
}

#[tauri::command]
pub async fn set_library_folder(
    app: AppHandle,
    path: String,
    recursive: bool,
) -> AppResult<ScanSummary> {
    blocking(move || {
        let root = PathBuf::from(&path);
        if !root.is_absolute() {
            return Err(AppError::invalid_input("Caminho de pasta inválido."));
        }
        if !root.is_dir() {
            return Err(AppError::new(
                "library_folder_not_found",
                format!("A pasta {path} não foi encontrada ou não está acessível."),
            ));
        }
        // B6 (security review): never index (and serve) a whole drive.
        if root.parent().is_none() {
            return Err(AppError::invalid_input(
                "Escolha uma pasta de músicas, não a unidade inteira.",
            ));
        }
        let summary = run_scan(&app, &root, recursive)?;
        let state = app.state::<AppState>();
        settings::write(
            &mut lock(state.db()?),
            json!({"libraryFolder": path, "recursiveScan": recursive})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        )?;
        start_watcher(&app, &root, recursive);
        Ok(summary)
    })
    .await
}

#[tauri::command]
pub async fn rescan_library(app: AppHandle) -> AppResult<ScanSummary> {
    blocking(move || {
        let s = settings::load(&lock(app.state::<AppState>().db()?))?;
        let Some(folder) = s.library_folder else {
            return Err(AppError::new(
                "no_library_folder",
                "Nenhuma pasta de biblioteca foi escolhida.",
            ));
        };
        run_scan(&app, Path::new(&folder), s.recursive_scan)
    })
    .await
}

#[tauri::command]
pub async fn list_songs(state: State<'_, AppState>) -> AppResult<Vec<Song>> {
    library::list_songs(&lock(state.db()?))
}

#[tauri::command]
pub async fn update_song(
    state: State<'_, AppState>,
    id: String,
    patch: SongPatch,
) -> AppResult<Song> {
    library::update_song(&lock(state.db()?), &id, patch)
}

/// Startup (SPEC-002 R11): the saved catalog is served immediately; scope, rescan and
/// watcher are restored in the background.
pub fn restore_library(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let result = (|| -> AppResult<()> {
            let s = settings::load(&lock(app.state::<AppState>().db()?))?;
            let Some(folder) = s.library_folder else {
                return Ok(());
            };
            let root = PathBuf::from(folder);
            allow_library_files(&app)?; // saved catalog playable before the rescan ends
            run_scan(&app, &root, s.recursive_scan)?;
            start_watcher(&app, &root, s.recursive_scan);
            Ok(())
        })();
        if let Err(e) = result {
            log::warn!("library restore failed: {e}");
        }
    });
}

pub async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| {
        log::error!("background task failed: {e}");
        AppError::new("internal", "Falha interna ao processar a biblioteca.")
    })?
}

/// Law 09: the asset protocol serves only the indexed video files (scanner output: supported
/// extensions inside the library folder), never the whole folder. Tauri's runtime scope API
/// escapes patterns, so files are allowed one by one; patterns live in a set (no duplicates).
// ponytail: one pattern per song, linear match per request; fine for thousands of songs.
fn allow_library_files<R: Runtime>(app: &AppHandle<R>) -> AppResult<()> {
    let state = app.state::<AppState>();
    let paths: Vec<String> = {
        let conn = lock(state.db()?);
        let mut stmt = conn.prepare("SELECT path FROM songs WHERE available = 1")?;
        stmt.query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    let scope = app.asset_protocol_scope();
    for p in &paths {
        if let Err(e) = scope.allow_file(p) {
            log::warn!("asset scope for {p}: {e}");
        }
    }
    Ok(())
}

fn run_scan<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    recursive: bool,
) -> AppResult<ScanSummary> {
    let state = app.state::<AppState>();
    let _guard = state
        .scan_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (summary, change) = library::scan(state.db()?, root, recursive)?;
    allow_library_files(app)?;
    if !change.is_empty()
        && let Err(e) = app.emit("library://changed", &change)
    {
        log::warn!("emit library://changed: {e}");
    }
    Ok(summary)
}

/// Replaces the current watcher. Any non-access event under the folder triggers a rescan
/// (cheap: unchanged files are not re-hashed), which emits `library://changed`.
fn start_watcher(app: &AppHandle, root: &Path, recursive: bool) {
    let handle = app.clone();
    let scan_root = root.to_path_buf();
    let debouncer = new_debouncer(
        Duration::from_millis(1500),
        None,
        move |res: DebounceEventResult| match res {
            Ok(events)
                if events
                    .iter()
                    .any(|e| !matches!(e.kind, EventKind::Access(_))) =>
            {
                if let Err(e) = run_scan(&handle, &scan_root, recursive) {
                    log::warn!("watcher rescan failed: {e}");
                }
            }
            Ok(_) => {}
            Err(errors) => log::warn!("watcher errors: {errors:?}"),
        },
    );
    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    let watcher = debouncer.and_then(|mut d| d.watch(root, mode).map(|_| d));
    let state = app.state::<AppState>();
    let mut slot = state
        .watcher
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match watcher {
        Ok(w) => *slot = Some(w),
        Err(e) => {
            *slot = None;
            log::warn!("cannot watch {}: {e}", root.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_scope_allows_only_indexed_videos() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("song.mp4"), b"video").unwrap();
        std::fs::write(lib.join("secret.txt"), b"x").unwrap();
        let app = tauri::test::mock_app();
        app.manage(AppState::new(crate::db::open(&dir.path().join("t.db"))));
        run_scan(app.handle(), &lib, true).unwrap();
        let scope = app.asset_protocol_scope();
        assert!(scope.is_allowed(lib.join("song.mp4")));
        assert!(!scope.is_allowed(lib.join("secret.txt")));
        assert!(!scope.is_allowed(&lib));
    }
}
