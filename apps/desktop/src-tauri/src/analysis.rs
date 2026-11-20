//! Automatic reference and alignment (SPEC-012, T-012-03): separation model management,
//! in-memory song audio (raw IPC upload, never on disk), generation job and auto-align.
//! The analysis itself lives in the `song-analysis` crate (Agent 2).

use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::Connection;
use scoring_engine::Note;
use serde::Serialize;
use song_analysis::{AnalysisError, KnownModel, Separator};
use tauri::ipc::{Channel, InvokeBody, Request};
use tauri::{AppHandle, Manager, State};

use crate::commands::{AppState, blocking};
use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::library::{self, lock};
use crate::reference::{self, Reference};
use crate::settings::{self, SeparationModel};

/// The WebView decodes the video to mono 44.1 kHz f32 (SPEC-012 R6).
const SAMPLE_RATE: u32 = 44_100;
/// Upload limit: 20 min of audio (~212 MB of f32).
const MAX_AUDIO_SAMPLES: usize = 20 * 60 * SAMPLE_RATE as usize;
/// Fewer notes than this is not a usable reference (same rule as the sung reference).
const MIN_NOTES: usize = 3;
/// How long a new job waits for a cancelled one to stop (it checks between ~4 s chunks).
const CANCELLED_JOB_WAIT: Duration = Duration::from_secs(30);
const REASON_NO_VOICE: &str = "Este vídeo não tem a voz da melodia (é só instrumental). Use Gravar cantando ou importe um MIDI.";
const REASON_FEW_NOTES: &str =
    "Quase nenhuma nota foi encontrada na voz do vídeo. Use Gravar cantando ou importe um MIDI.";

/// Mirrors `KnownSeparationModel` in ipc.ts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownSeparationModel {
    pub id: &'static str,
    pub name: &'static str,
    pub file_name: &'static str,
    pub size_bytes: u64,
    pub source_url: &'static str,
    pub license: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Separating,
    Transcribing,
    Saving,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoReferenceProgress {
    pub stage: Stage,
    pub fraction: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoReferenceOutcome {
    /// New active reference (`extracted`, `experimental`); `None` = nothing saved.
    pub reference: Option<Reference>,
    pub reason: Option<String>,
    pub voice_level_db: f32,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignProposal {
    /// Proposed new offset (current offset included). Nothing is saved.
    pub offset_ms: i32,
    pub confidence: f32,
}

struct SongAudio {
    token: String,
    song_id: String,
    samples: Arc<Vec<f32>>,
}

/// One uploaded song audio (replaced by the next upload) and one generation job at a time.
#[derive(Default)]
pub struct AnalysisState {
    audio: Mutex<Option<SongAudio>>,
    busy: AtomicBool,
    cancel: AtomicBool,
}

impl AnalysisState {
    fn put(&self, song_id: String, samples: Vec<f32>) -> String {
        // Unguessable enough for a local IPC handle; not a secret.
        let s = std::collections::hash_map::RandomState::new();
        let token = format!("{:016x}{:016x}", s.hash_one(now_ms()), s.hash_one(&song_id));
        *self
            .audio
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(SongAudio {
            token: token.clone(),
            song_id,
            samples: Arc::new(samples),
        });
        token
    }

    fn get(&self, token: &str, song_id: &str) -> AppResult<Arc<Vec<f32>>> {
        self.audio
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|a| a.token == token && a.song_id == song_id)
            .map(|a| a.samples.clone())
            .ok_or_else(|| {
                AppError::new(
                    "audio_token_invalid",
                    "O áudio desta música não está mais carregado. Tente de novo.",
                )
            })
    }

    /// Takes the single job slot. A cancelled job still finishing its current chunk is waited
    /// for (never two separators at once: ~1.2 GB each); a live job → `busy`.
    fn acquire(&self, wait: Duration) -> AppResult<JobGuard<'_>> {
        let deadline = Instant::now() + wait;
        while self
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            if !self.cancel.load(Ordering::SeqCst) || Instant::now() > deadline {
                return Err(AppError::new(
                    "busy",
                    "Já existe uma referência sendo gerada. Aguarde ou cancele.",
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.cancel.store(false, Ordering::SeqCst);
        Ok(JobGuard(&self.busy))
    }
}

/// Clears `busy` even if the job panics.
#[derive(Debug)]
struct JobGuard<'a>(&'a AtomicBool);
impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn analysis_err(e: AnalysisError) -> AppError {
    match e {
        AnalysisError::Cancelled => AppError::new("cancelled", "Geração cancelada."),
        e => {
            log::error!("song analysis failed: {e}");
            AppError::new("analysis_failed", e.to_string())
        }
    }
}

/// f32 little-endian mono samples; rejects empty, ragged, oversized or non-finite input.
fn parse_audio(bytes: &[u8]) -> AppResult<Vec<f32>> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(AppError::new("audio_invalid", "Áudio inválido."));
    }
    if bytes.len() / 4 > MAX_AUDIO_SAMPLES {
        return Err(AppError::new(
            "audio_invalid",
            "Música longa demais para a análise automática (máximo 20 minutos).",
        ));
    }
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect();
    if samples.iter().any(|s| !s.is_finite()) {
        return Err(AppError::new("audio_invalid", "Áudio inválido."));
    }
    Ok(samples)
}

fn models_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("models"))
        .map_err(|e| AppError::new("io", format!("Pasta de dados indisponível: {e}")))
}

fn model_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.onnx"))
}

/// Checks size, then SHA-256, against the known list (R1: only known models), copies the file
/// into `models_dir` and records it in settings. Replaces a previously installed model.
pub fn install(
    conn: &mut Connection,
    models_dir: &Path,
    file_path: &str,
    accept_terms: bool,
    known: &[KnownModel],
) -> AppResult<SeparationModel> {
    if !accept_terms {
        return Err(AppError::new(
            "terms_not_accepted",
            "Para instalar o modelo, confirme que leu e aceita os termos da fonte.",
        ));
    }
    let src = Path::new(file_path);
    if !src.is_absolute() {
        return Err(AppError::invalid_input("Caminho de arquivo inválido."));
    }
    let meta = std::fs::metadata(src)
        .map_err(|_| AppError::new("not_found", format!("Arquivo não encontrado: {file_path}.")))?;
    let unknown = || {
        AppError::new(
            "model_unknown",
            "Este arquivo não é um dos modelos conhecidos (o conteúdo não confere). Baixe um modelo da lista, na fonte indicada, sem alterá-lo.",
        )
    };
    // Exact size first: cheap rejection before hashing tens of MB.
    if !meta.is_file() || !known.iter().any(|m| m.size_bytes == meta.len()) {
        return Err(unknown());
    }
    let hash = song_analysis::sha256_file(src)?;
    let model = known.iter().find(|m| m.id == hash).ok_or_else(unknown)?;

    std::fs::create_dir_all(models_dir)?;
    let dest = model_file(models_dir, model.id);
    let tmp = dest.with_extension("onnx.part");
    std::fs::copy(src, &tmp)?;
    std::fs::rename(&tmp, &dest)?;

    let previous = settings::load(conn)?.separation_model;
    let installed = SeparationModel {
        id: model.id.to_owned(),
        name: model.name.to_owned(),
        accepted_ms: now_ms(),
    };
    let mut patch = serde_json::Map::new();
    patch.insert("separationModel".into(), serde_json::to_value(&installed)?);
    settings::write(conn, patch)?;
    if let Some(old) = previous.filter(|o| o.id != installed.id) {
        remove_file(&model_file(models_dir, &old.id))?;
    }
    log::info!("separation model installed: {}", model.name);
    Ok(installed)
}

pub fn remove(conn: &mut Connection, models_dir: &Path) -> AppResult<()> {
    if let Some(m) = settings::load(conn)?.separation_model {
        remove_file(&model_file(models_dir, &m.id))?;
    }
    let mut patch = serde_json::Map::new();
    patch.insert("separationModel".into(), serde_json::Value::Null);
    settings::write(conn, patch)?;
    Ok(())
}

fn remove_file(path: &Path) -> AppResult<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Stores the transcribed notes, or explains why nothing was saved.
pub fn store_generated(
    conn: &mut Connection,
    song_id: &str,
    notes: Vec<Note>,
) -> AppResult<(Option<Reference>, Option<String>)> {
    if notes.len() < MIN_NOTES {
        return Ok((None, Some(REASON_FEW_NOTES.to_owned())));
    }
    let r = reference::store_notes(conn, song_id, "extracted", notes)?;
    Ok((Some(r), None))
}

/// Reference notes as heard in the video: shifted by the current offset, those before 0 dropped.
fn shifted_notes(r: &Reference) -> Vec<Note> {
    r.notes
        .iter()
        .filter_map(|n| {
            let start = i64::from(n.start_ms) + i64::from(r.offset_ms);
            Some(Note {
                start_ms: u32::try_from(start).ok()?,
                dur_ms: n.dur_ms,
                midi: n.midi,
                lyric: None,
            })
        })
        .collect()
}

#[tauri::command]
pub async fn list_known_separation_models() -> AppResult<Vec<KnownSeparationModel>> {
    Ok(song_analysis::known_models()
        .iter()
        .map(|m| KnownSeparationModel {
            id: m.id,
            name: m.name,
            file_name: m.file_name,
            size_bytes: m.size_bytes,
            source_url: m.source_url,
            license: m.license,
        })
        .collect())
}

#[tauri::command]
pub async fn install_separation_model(
    app: AppHandle,
    file_path: String,
    accept_terms: bool,
) -> AppResult<SeparationModel> {
    // Hashing and copying tens of MB: off the async workers.
    blocking(move || {
        let dir = models_dir(&app)?;
        let state = app.state::<AppState>();
        let mut conn = lock(state.db()?);
        install(
            &mut conn,
            &dir,
            &file_path,
            accept_terms,
            song_analysis::known_models(),
        )
    })
    .await
}

#[tauri::command]
pub async fn remove_separation_model(app: AppHandle) -> AppResult<()> {
    let dir = models_dir(&app)?;
    remove(&mut lock(app.state::<AppState>().db()?), &dir)
}

/// Raw IPC: body = f32 LE mono 44.1 kHz, header `song-id`. Kept only in memory.
#[tauri::command]
pub async fn upload_song_audio(
    state: State<'_, AppState>,
    analysis: State<'_, AnalysisState>,
    request: Request<'_>,
) -> AppResult<String> {
    let song_id = request
        .headers()
        .get("song-id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::new("audio_invalid", "Música não informada."))?
        .to_owned();
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(AppError::new("audio_invalid", "Áudio inválido."));
    };
    let samples = parse_audio(bytes)?;
    library::get_song(&lock(state.db()?), &song_id)?;
    log::info!(
        "song audio loaded: {song_id}, {:.1} s",
        samples.len() as f64 / f64::from(SAMPLE_RATE)
    );
    Ok(analysis.put(song_id, samples))
}

#[tauri::command]
pub async fn generate_reference(
    app: AppHandle,
    song_id: String,
    audio_token: String,
    on_progress: Channel<AutoReferenceProgress>,
) -> AppResult<AutoReferenceOutcome> {
    blocking(move || {
        let analysis = app.state::<AnalysisState>();
        let _guard = analysis.acquire(CANCELLED_JOB_WAIT)?;
        let started = Instant::now();
        let progress = |stage, fraction| {
            let _ = on_progress.send(AutoReferenceProgress { stage, fraction }); // UI gone: keep going
        };

        let mix = analysis.get(&audio_token, &song_id)?;
        let state = app.state::<AppState>();
        let installed = settings::load(&lock(state.db()?))?.separation_model;
        let missing = || {
            AppError::new(
                "model_not_installed",
                "Instale o modelo de separação em Ajustes → Referência automática.",
            )
        };
        let model = installed
            .and_then(|m| song_analysis::known_models().iter().find(|k| k.id == m.id))
            .ok_or_else(missing)?;
        let path = model_file(&models_dir(&app)?, model.id);
        if !path.is_file() {
            return Err(missing());
        }

        progress(Stage::Separating, 0.0);
        let mut separator = Separator::load(&path, model).map_err(analysis_err)?;
        let vocals = separator
            .vocals(
                &mix,
                SAMPLE_RATE,
                &mut |f| progress(Stage::Separating, f),
                &analysis.cancel,
            )
            .map_err(analysis_err)?;
        drop(separator);
        let voice_level_db = song_analysis::voice_level_db(&vocals, &mix);
        let outcome =
            |(reference, reason): (Option<Reference>, Option<String>)| AutoReferenceOutcome {
                reference,
                reason,
                voice_level_db,
                elapsed_ms: started.elapsed().as_millis() as u64,
            };
        if voice_level_db < song_analysis::NO_VOICE_DB {
            log::info!("auto reference: no voice ({voice_level_db:.1} dB)");
            return Ok(outcome((None, Some(REASON_NO_VOICE.to_owned()))));
        }

        progress(Stage::Transcribing, 0.0);
        let notes = song_analysis::transcribe(&vocals, SAMPLE_RATE);
        if analysis.cancel.load(Ordering::SeqCst) {
            return Err(analysis_err(AnalysisError::Cancelled));
        }
        progress(Stage::Transcribing, 1.0);

        progress(Stage::Saving, 0.0);
        let saved = store_generated(&mut lock(state.db()?), &song_id, notes)?;
        progress(Stage::Saving, 1.0);
        let out = outcome(saved);
        log::info!(
            "auto reference: saved {}, voice {:.1} dB, {} ms",
            out.reference.is_some(),
            out.voice_level_db,
            out.elapsed_ms
        );
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn cancel_generate_reference(analysis: State<'_, AnalysisState>) -> AppResult<()> {
    analysis.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn auto_align_reference(
    app: AppHandle,
    reference_id: i64,
    audio_token: String,
) -> AppResult<AlignProposal> {
    blocking(move || {
        let r = reference::get_active_by_id(&lock(app.state::<AppState>().db()?), reference_id)?;
        let mix = app.state::<AnalysisState>().get(&audio_token, &r.song_id)?;
        let notes = shifted_notes(&r);
        if notes.is_empty() {
            return Err(AppError::invalid_input(
                "A referência não tem notas para alinhar.",
            ));
        }
        let res = song_analysis::align_offset_ms(&notes, &mix, SAMPLE_RATE);
        Ok(AlignProposal {
            offset_ms: r.offset_ms + res.offset_ms,
            confidence: res.confidence,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        conn.execute(
            "INSERT INTO songs (id, path, file_name, title, extension, size_bytes, modified_ms, added_ms, updated_ms)
             VALUES ('s1', 'C:/v/s1.mp4', 's1.mp4', 's1', 'mp4', 1, 1, 1, 1)",
            [],
        )
        .unwrap();
        (dir, conn)
    }

    fn fake_model(id: &'static str, size: u64) -> KnownModel {
        KnownModel {
            id,
            name: "Modelo de teste",
            file_name: "test.onnx",
            size_bytes: size,
            source_url: "https://example.invalid/model",
            license: "teste",
            sample_rate: SAMPLE_RATE,
        }
    }

    fn note(start_ms: u32) -> Note {
        Note {
            start_ms,
            dur_ms: 400,
            midi: 60.0,
            lyric: None,
        }
    }

    #[test]
    fn install_checks_terms_size_and_hash_then_remove_cleans_up() {
        let (dir, mut conn) = env();
        let models = dir.path().join("models");
        let file = dir.path().join("dl.onnx");
        std::fs::write(&file, b"not really a model").unwrap();
        let hash: &'static str = song_analysis::sha256_file(&file).unwrap().leak();
        let path = file.to_str().unwrap();
        let good = [fake_model(hash, 18)];

        let err = |r: AppResult<SeparationModel>| r.unwrap_err().code;
        assert_eq!(
            err(install(&mut conn, &models, path, false, &good)),
            "terms_not_accepted"
        );
        assert_eq!(
            err(install(&mut conn, &models, "rel.onnx", true, &good)),
            "invalid_input"
        );
        let wrong_size = [fake_model(hash, 19)];
        assert_eq!(
            err(install(&mut conn, &models, path, true, &wrong_size)),
            "model_unknown"
        );
        let wrong_hash = [fake_model("00", 18)];
        assert_eq!(
            err(install(&mut conn, &models, path, true, &wrong_hash)),
            "model_unknown"
        );
        assert!(!models.exists(), "nothing copied when refused");
        assert_eq!(settings::load(&conn).unwrap().separation_model, None);

        let m = install(&mut conn, &models, path, true, &good).unwrap();
        assert_eq!((m.id.as_str(), m.name.as_str()), (hash, "Modelo de teste"));
        assert_eq!(
            std::fs::read(model_file(&models, hash)).unwrap(),
            b"not really a model"
        );
        assert_eq!(settings::load(&conn).unwrap().separation_model, Some(m));
        let json = serde_json::to_value(settings::load(&conn).unwrap()).unwrap();
        assert!(json["separationModel"]["acceptedMs"].is_i64());

        // Read-only for update_settings.
        let patch = serde_json::json!({"separationModel": null});
        assert_eq!(
            settings::update(&mut conn, patch.as_object().cloned().unwrap())
                .unwrap_err()
                .code,
            "invalid_setting"
        );

        remove(&mut conn, &models).unwrap();
        assert!(!model_file(&models, hash).exists());
        assert_eq!(settings::load(&conn).unwrap().separation_model, None);
        remove(&mut conn, &models).unwrap(); // idempotent
    }

    #[test]
    fn audio_upload_is_validated_and_kept_once() {
        let bytes: Vec<u8> = [0.5f32, -0.25]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        assert_eq!(parse_audio(&bytes).unwrap(), vec![0.5, -0.25]);
        assert!(parse_audio(&[]).is_err());
        assert!(parse_audio(&bytes[..7]).is_err());
        assert!(parse_audio(&f32::NAN.to_le_bytes()).is_err());
        assert!(parse_audio(&vec![0u8; (MAX_AUDIO_SAMPLES + 1) * 4]).is_err());

        let st = AnalysisState::default();
        let t1 = st.put("s1".into(), vec![1.0]);
        assert_eq!(*st.get(&t1, "s1").unwrap(), vec![1.0]);
        assert_eq!(st.get(&t1, "s2").unwrap_err().code, "audio_token_invalid");
        let t2 = st.put("s2".into(), vec![2.0]);
        assert_ne!(t1, t2);
        assert_eq!(
            st.get(&t1, "s1").unwrap_err().code,
            "audio_token_invalid",
            "replaced"
        );
        assert_eq!(*st.get(&t2, "s2").unwrap(), vec![2.0]);
    }

    #[test]
    fn new_job_waits_for_a_cancelled_one_but_not_for_a_live_one() {
        let st = AnalysisState::default();
        // Live job: a second generate is refused at once.
        let guard = st.acquire(Duration::from_secs(5)).unwrap();
        let t = Instant::now();
        assert_eq!(st.acquire(Duration::from_secs(5)).unwrap_err().code, "busy");
        assert!(t.elapsed() < Duration::from_secs(1));
        drop(guard);

        // Cancel, then a new generate right away: it waits for the old job to see the flag.
        std::thread::scope(|s| {
            let job = st.acquire(Duration::from_secs(5)).unwrap();
            let old = s.spawn(|| {
                let _job = job;
                // A chunk in progress; the flag is only checked between chunks.
                std::thread::sleep(Duration::from_millis(200));
                st.cancel.load(Ordering::SeqCst)
            });
            st.cancel.store(true, Ordering::SeqCst); // cancel_generate_reference
            let new = st
                .acquire(Duration::from_secs(5))
                .expect("no busy after cancel");
            assert!(old.join().unwrap(), "old job saw the cancel (→ cancelled)");
            assert!(
                !st.cancel.load(Ordering::SeqCst),
                "new job starts uncancelled"
            );
            drop(new);
        });
        assert!(!st.busy.load(Ordering::SeqCst));
    }

    #[test]
    fn generated_reference_is_extracted_experimental_and_active() {
        let (_d, mut conn) = env();
        let old = reference::store_notes(
            &mut conn,
            "s1",
            "manual",
            vec![note(0), note(500), note(1000)],
        )
        .unwrap();

        let (r, reason) = store_generated(&mut conn, "s1", vec![note(100), note(600)]).unwrap();
        assert_eq!((r, reason.as_deref()), (None, Some(REASON_FEW_NOTES)));
        assert_eq!(reference::active(&conn, "s1").unwrap().unwrap().id, old.id);

        let (r, reason) =
            store_generated(&mut conn, "s1", vec![note(100), note(600), note(1100)]).unwrap();
        let r = r.unwrap();
        assert_eq!(reason, None);
        assert_eq!(
            (
                r.source.as_str(),
                r.status.as_str(),
                r.revision,
                r.offset_ms
            ),
            ("extracted", "experimental", 1, 0)
        );
        assert_eq!(reference::active(&conn, "s1").unwrap(), Some(r));
    }

    #[test]
    fn align_uses_notes_as_heard_in_the_video() {
        let (_d, mut conn) = env();
        let r = reference::store_notes(
            &mut conn,
            "s1",
            "json",
            vec![note(100), note(500), note(900)],
        )
        .unwrap();
        let r = reference::set_offset(&conn, r.id, -300).unwrap();
        let starts: Vec<u32> = shifted_notes(&r).iter().map(|n| n.start_ms).collect();
        assert_eq!(starts, vec![200, 600], "note before 0 dropped");
    }
}
