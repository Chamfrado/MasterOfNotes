//! Live performance commands and persistence (SPEC-006 T-006-05, SPEC-007 R1/R2/R5).
//! Session logic (anchor, sync, scorer) lives in `performance.rs` (Agent 2); this module
//! decides what is competitive, stores the result and computes the records.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use game::CompatKey;
use rusqlite::{Connection, OptionalExtension};
use scoring_engine::{ExploratoryMetrics, ScoreResult};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::commands::{AppState, blocking};
use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::library::{self, lock};
use crate::mic::MicMonitor;
use crate::performance::{
    LatencyVerdict, LeakVerdict, LivePerformance, LiveTick, PerformanceConfig, PerformanceSummary,
};
use crate::reference;
use crate::sessions::{self, difficulty_str};
use crate::settings::{self, Difficulty};

/// SPEC-007 R2: at least this fraction of the song must have been played.
const MIN_PLAYED_RATIO: f64 = 0.5;
/// Device-error polling period (watchdog for `audio://error`).
const WATCH_EVERY: Duration = Duration::from_millis(250);
/// Upper bound on latency-probe clicks accepted from the UI (Law 09).
const MAX_PROBE_CLICKS: usize = 500;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartPerformance {
    pub song_id: String,
    pub round_id: Option<i64>,
    pub participant_id: Option<i64>,
    /// Leak check run (T-004-06): user silent, nothing stored in `performances`.
    #[serde(default)]
    pub leak_check: bool,
    /// Latency probe (T-004-07): click times in video ms. Exclusive with `leak_check`.
    #[serde(default)]
    pub latency_probe: Option<Vec<f64>>,
    /// Sung reference capture (T-005-07): the detected notes become the active reference.
    /// Exclusive with `leak_check` and `latency_probe`.
    #[serde(default)]
    pub reference_capture: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceStarted {
    pub competitive: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceOutcome {
    pub performance_id: Option<i64>,
    pub competitive: bool,
    pub result: Option<ScoreResult>,
    pub exploratory: Option<ExploratoryMetrics>,
    /// Only on leak check runs.
    pub leak: Option<LeakVerdict>,
    /// Only on latency probe runs.
    pub latency: Option<LatencyVerdict>,
    /// Only on reference capture runs: the new active reference (`None` = too few notes).
    pub captured_reference: Option<reference::Reference>,
    pub personal_best: Option<f32>,
    pub song_record: Option<f32>,
    /// pt-BR: why the run did not count for the ranking (`None` when competitive).
    pub reason: Option<String>,
}

const REASON_HALF: &str = "Menos da metade da música foi tocada: a apresentação valeu como treino.";
const REASON_NO_DURATION: &str =
    "A duração da música ainda não é conhecida: a apresentação valeu como treino.";
const REASON_TRAINING: &str = "A apresentação valeu como treino.";
/// A sung reference needs at least this many notes to be saved (T-005-07).
const MIN_CAPTURED_NOTES: usize = 3;
const REASON_FEW_NOTES: &str =
    "Poucas notas detectadas: cante a melodia mais perto do microfone e tente de novo.";

/// What the run needs to be stored when it finishes (captured at start).
#[derive(Debug, Clone, PartialEq)]
pub struct RunContext {
    pub song_id: String,
    pub round_id: Option<i64>,
    pub participant_id: Option<i64>,
    /// (reference id, revision) of the active reference at start.
    pub reference: Option<(i64, i64)>,
    pub difficulty: Difficulty,
    /// Leak check run: (device id, video volume) at start, saved with the verdict.
    pub leak_check: Option<(Option<String>, f64)>,
    /// Latency probe run (T-004-07).
    pub latency_probe: bool,
    /// Sung reference capture run (T-005-07).
    pub reference_capture: bool,
    /// Why the run was not competitive from the start (the `reason` of `PerformanceStarted`).
    pub start_reason: Option<String>,
}

struct Running {
    generation: u64,
    perf: LivePerformance,
    ctx: RunContext,
}

/// The single live performance. Drop of `LivePerformance` = abort.
#[derive(Default)]
pub struct LiveState {
    running: Mutex<Option<Running>>,
    generation: AtomicU64,
}

fn running(state: &LiveState) -> std::sync::MutexGuard<'_, Option<Running>> {
    lock(&state.running)
}

/// Polls a device error every 250 ms. `check` returns `None` when the capture it watches is
/// gone (stop/replace → watchdog exits) and `Some(Some(err))` after it has stopped the
/// capture because of `err`; the error is then emitted as `audio://error`.
pub fn spawn_error_watch<R: Runtime>(
    app: AppHandle<R>,
    mut check: impl FnMut(&AppHandle<R>) -> Option<Option<AppError>> + Send + 'static,
) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(WATCH_EVERY);
            match check(&app) {
                None => return,
                Some(None) => {}
                Some(Some(e)) => {
                    log::error!("audio device failed: {e}");
                    if let Err(err) = app.emit("audio://error", &e) {
                        log::warn!("emit audio://error: {err}");
                    }
                    return;
                }
            }
        }
    });
}

/// Validates the request and builds the session config. No device is touched.
pub fn prepare(
    conn: &Connection,
    args: &StartPerformance,
) -> AppResult<(PerformanceConfig, RunContext, PerformanceStarted)> {
    library::get_available_song(conn, &args.song_id)?;
    if let Some(pid) = args.participant_id {
        sessions::require_participant(conn, pid)?;
    }
    if let Some(round_id) = args.round_id {
        let (song_id, participant_id, status, session_status): (
            String,
            Option<i64>,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT r.song_id, r.participant_id, r.status, s.status FROM rounds r
                 JOIN game_sessions s ON s.id = r.session_id WHERE r.id = ?1",
                [round_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
            .ok_or_else(|| AppError::new("not_found", "Rodada não encontrada."))?;
        if session_status != "active" || status == "completed" {
            return Err(AppError::invalid_input(
                "Esta rodada não está mais disponível.",
            ));
        }
        if song_id != args.song_id || participant_id != args.participant_id {
            return Err(AppError::invalid_input(
                "A música ou o participante não correspondem à rodada.",
            ));
        }
    }

    let s = settings::load(conn)?;
    let active = reference::active(conn, &args.song_id)?;
    if args.leak_check {
        if args.round_id.is_some() || args.participant_id.is_some() {
            return Err(AppError::invalid_input(
                "O teste de vazamento não pertence a uma rodada nem a um participante.",
            ));
        }
        // The verdict compares the mic against the reference notes.
        if !active.as_ref().is_some_and(|r| r.status != "invalid") {
            return Err(AppError::invalid_input(
                "Escolha uma música com referência melódica para o teste de vazamento.",
            ));
        }
    }
    if let Some(clicks) = &args.latency_probe {
        if args.leak_check || args.round_id.is_some() || args.participant_id.is_some() {
            return Err(AppError::invalid_input(
                "A sonda de latência roda sozinha: sem teste de vazamento, rodada ou participante.",
            ));
        }
        if clicks.is_empty() || clicks.len() > MAX_PROBE_CLICKS {
            return Err(AppError::invalid_input(
                "A sonda de latência precisa de 1 a 500 cliques.",
            ));
        }
    }
    let capture_run = args.reference_capture;
    if capture_run
        && (args.leak_check
            || args.latency_probe.is_some()
            || args.round_id.is_some()
            || args.participant_id.is_some())
    {
        return Err(AppError::invalid_input(
            "A gravação da referência roda sozinha: sem teste de vazamento, sonda, rodada ou participante.",
        ));
    }
    let probe = args.latency_probe.is_some();
    let (competitive, reason) = match active.as_ref().map(|r| r.status.as_str()) {
        _ if capture_run => (
            false,
            Some("Gravação da referência: cante a melodia da música."),
        ),
        _ if probe => (
            false,
            Some("Sonda de latência: bata palmas junto com os cliques."),
        ),
        _ if args.leak_check => (
            false,
            Some("Teste de vazamento: fique em silêncio enquanto a música toca."),
        ),
        // SPEC-006 A2: the guide vocal must not leak into the microphone.
        Some("validated") if s.leak_check_ok() => (true, None),
        Some("validated") => (
            false,
            Some("Faça o teste de vazamento do microfone (ou use fones) para pontuar no ranking."),
        ),
        Some("invalid") => (
            false,
            Some(
                "A referência desta música foi marcada como inválida: a apresentação é só treino.",
            ),
        ),
        Some(_) => (
            false,
            Some("A referência desta música ainda não foi validada: a apresentação é só treino."),
        ),
        None => (
            false,
            Some(
                "Esta música não tem referência melódica: não há pontuação, só métricas de treino.",
            ),
        ),
    };
    let reference = match &active {
        // The probe only listens for clicks; a capture writes a new reference.
        Some(_) if probe || capture_run => None,
        Some(r) => {
            let notes_json: String = conn.query_row(
                "SELECT notes_json FROM melody_references WHERE id = ?1",
                [r.id],
                |x| x.get(0),
            )?;
            Some((reference::parse_stored(&notes_json)?, r.offset_ms))
        }
        None => None,
    };
    let cfg = PerformanceConfig {
        reference,
        competitive,
        difficulty: s.difficulty,
        // The probe measures the raw latency, so it runs without the current correction.
        latency_offset_ms: if probe {
            0.0
        } else {
            s.latency_offset_ms as f64
        },
        device_id: s.input_device_id.clone(),
        capture: s.capture_config(),
        leak_check: args.leak_check,
        latency_probe: args.latency_probe.clone(),
        reference_capture: capture_run,
    };
    let ctx = RunContext {
        song_id: args.song_id.clone(),
        round_id: args.round_id,
        participant_id: args.participant_id,
        reference: active
            .as_ref()
            .filter(|_| !capture_run)
            .map(|r| (r.id, r.revision)),
        difficulty: s.difficulty,
        leak_check: args
            .leak_check
            .then(|| (s.input_device_id.clone(), s.video_volume)),
        latency_probe: probe,
        reference_capture: capture_run,
        start_reason: reason.map(str::to_owned),
    };
    Ok((
        cfg,
        ctx,
        PerformanceStarted {
            competitive,
            reason: reason.map(str::to_owned),
        },
    ))
}

/// Stores a finished run. Without a reference nothing is stored (SPEC-007 R1).
pub fn persist(
    conn: &mut Connection,
    ctx: &RunContext,
    summary: PerformanceSummary,
) -> AppResult<PerformanceOutcome> {
    let not_stored = PerformanceOutcome {
        performance_id: None,
        competitive: false,
        result: None,
        exploratory: None,
        leak: None,
        latency: None,
        captured_reference: None,
        personal_best: None,
        song_record: None,
        reason: None,
    };
    // Sung reference (T-005-07): only the notes are kept, as the new active reference.
    if ctx.reference_capture {
        let notes = summary.captured_notes.unwrap_or_default();
        log::info!("reference capture: {} notes detected", notes.len());
        if notes.len() < MIN_CAPTURED_NOTES {
            return Ok(PerformanceOutcome {
                exploratory: summary.exploratory,
                reason: Some(REASON_FEW_NOTES.to_owned()),
                ..not_stored
            });
        }
        return Ok(PerformanceOutcome {
            exploratory: summary.exploratory,
            captured_reference: Some(reference::store_notes(conn, &ctx.song_id, "manual", notes)?),
            ..not_stored
        });
    }
    // Latency probe (T-004-07): a passed measurement becomes the latency setting.
    if ctx.latency_probe {
        let Some(verdict) = summary.latency else {
            return Err(AppError::new(
                "internal",
                "A sonda de latência não produziu resultado.",
            ));
        };
        if verdict.passed {
            let mut patch = serde_json::Map::new();
            patch.insert(
                "latencyOffsetMs".into(),
                serde_json::json!(verdict.result.offset_ms.round() as i64),
            );
            settings::write(conn, patch)?;
        }
        log::info!(
            "latency probe: passed {} (offset {:.1} ms, jitter {:.1} ms, {}/{})",
            verdict.passed,
            verdict.result.offset_ms,
            verdict.result.jitter_ms,
            verdict.result.detected,
            verdict.result.expected
        );
        return Ok(PerformanceOutcome {
            latency: Some(verdict),
            ..not_stored
        });
    }
    // Leak check run: only the verdict is kept, in settings (T-004-06). Any non-passed
    // verdict replaces an older pass: competitive play needs the latest check to pass.
    if let Some((device_id, video_volume)) = &ctx.leak_check {
        let Some(verdict) = summary.leak else {
            return Err(AppError::new(
                "internal",
                "O teste de vazamento não produziu resultado.",
            ));
        };
        let check = settings::LeakCheck {
            device_id: device_id.clone(),
            video_volume: *video_volume,
            passed: verdict.passed,
            at_ms: now_ms(),
        };
        let mut patch = serde_json::Map::new();
        patch.insert("leakCheck".into(), serde_json::to_value(&check)?);
        settings::write(conn, patch)?;
        log::info!(
            "leak check: passed {} (guide score {:.3})",
            verdict.passed,
            verdict.guide_score
        );
        return Ok(PerformanceOutcome {
            leak: Some(verdict),
            ..not_stored
        });
    }
    let (Some(result), Some((ref_id, revision))) = (summary.result, ctx.reference) else {
        return Ok(PerformanceOutcome {
            exploratory: summary.exploratory,
            reason: ctx.start_reason.clone().or(summary.reason),
            ..not_stored
        });
    };
    let duration_ms: Option<i64> = library::get_song(conn, &ctx.song_id)?.duration_ms;
    // Unknown duration → cannot prove 50% was played → training.
    let played_enough = duration_ms
        .is_some_and(|d| d > 0 && summary.played_ms as f64 >= MIN_PLAYED_RATIO * d as f64);
    let competitive = summary.competitive && played_enough;
    // The Result screen always explains a training run: root cause known at start first
    // (no validated reference, leak check), then the session's (seek…), then this gate.
    let reason = if competitive {
        None
    } else if let Some(r) = &ctx.start_reason {
        Some(r.clone())
    } else if !summary.competitive {
        Some(summary.reason.unwrap_or_else(|| REASON_TRAINING.to_owned()))
    } else if duration_ms.is_none() {
        Some(REASON_NO_DURATION.to_owned())
    } else {
        Some(REASON_HALF.to_owned())
    };
    let difficulty = difficulty_str(ctx.difficulty);

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO performances (round_id, song_id, participant_id, reference_id, reference_revision, algorithm_version,
         difficulty, competitive, score, metrics_json, capture_json, created_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        (
            ctx.round_id,
            &ctx.song_id,
            ctx.participant_id,
            ref_id,
            revision,
            result.algorithm_version,
            difficulty,
            competitive,
            result.score as f64,
            serde_json::to_string(&result)?,
            serde_json::to_string(&summary.capture)?,
            now_ms(),
        ),
    )?;
    let performance_id = tx.last_insert_rowid();
    // A round completes only with a competitive performance (SPEC-008).
    if competitive && let Some(round_id) = ctx.round_id {
        tx.execute(
            "UPDATE rounds SET status = 'completed' WHERE id = ?1 AND status != 'completed'
             AND session_id IN (SELECT id FROM game_sessions WHERE status = 'active')",
            [round_id],
        )?;
    }
    tx.commit()?;

    let compat = CompatKey {
        reference_id: ref_id,
        reference_revision: revision,
        algorithm_version: result.algorithm_version,
        difficulty: difficulty.to_owned(),
    };
    let records = sessions::competitive_records(conn, &ctx.song_id, &compat)?;
    let best = |filter: &dyn Fn(Option<i64>) -> bool| {
        records
            .iter()
            .filter(|(_, _, r)| filter(r.participant_id))
            .map(|(_, _, r)| r.score)
            .reduce(f32::max)
    };
    Ok(PerformanceOutcome {
        performance_id: Some(performance_id),
        competitive,
        result: Some(result),
        personal_best: ctx.participant_id.and_then(|p| best(&|q| q == Some(p))),
        song_record: best(&|_| true),
        reason,
        ..not_stored
    })
}

#[tauri::command]
pub async fn start_performance(
    app: AppHandle,
    args: StartPerformance,
    on_tick: Channel<LiveTick>,
) -> AppResult<PerformanceStarted> {
    blocking(move || {
        let live = app.state::<LiveState>();
        // One microphone user at a time: replaces a forgotten run and the meter.
        running(&live).take();
        app.state::<MicMonitor>().stop();
        let (cfg, ctx, started) = prepare(&lock(app.state::<AppState>().db()?), &args)?;
        let perf = LivePerformance::start(cfg, move |t| {
            let _ = on_tick.send(t); // UI gone: the run continues until finish/abort
        })?;
        let generation = live.generation.fetch_add(1, Ordering::Relaxed) + 1;
        *running(&live) = Some(Running {
            generation,
            perf,
            ctx,
        });
        spawn_error_watch(app.clone(), move |app| {
            let live = app.state::<LiveState>();
            let mut slot = running(&live);
            let r = slot.as_ref().filter(|r| r.generation == generation)?;
            let err = r.perf.take_error()?;
            slot.take(); // stops the stream; the run is lost (not scored)
            Some(Some(err))
        });
        log::info!("performance started (competitive: {})", started.competitive);
        Ok(started)
    })
    .await
}

#[tauri::command]
pub async fn sync_performance(
    live: State<'_, LiveState>,
    video_time_s: f64,
    playing: bool,
    seeked: bool,
) -> AppResult<()> {
    match running(&live).as_ref() {
        Some(r) => {
            r.perf.sync(video_time_s, playing, seeked);
            Ok(())
        }
        None => Err(not_running()),
    }
}

#[tauri::command]
pub async fn finish_performance(app: AppHandle) -> AppResult<PerformanceOutcome> {
    blocking(move || {
        let r = running(&app.state::<LiveState>())
            .take()
            .ok_or_else(not_running)?;
        let summary = r.perf.finish();
        let outcome = persist(&mut lock(app.state::<AppState>().db()?), &r.ctx, summary)?;
        log::info!(
            "performance finished: id {:?}, competitive {}",
            outcome.performance_id,
            outcome.competitive
        );
        Ok(outcome)
    })
    .await
}

#[tauri::command]
pub async fn abort_performance(app: AppHandle) -> AppResult<()> {
    // Dropping the capture joins the audio thread (43–500 ms): keep it off the async workers.
    blocking(move || {
        running(&app.state::<LiveState>()).take();
        Ok(())
    })
    .await
}

fn not_running() -> AppError {
    AppError::new("no_performance", "Nenhuma apresentação em andamento.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::CaptureStats;

    const REF: &str =
        r#"{"format":"mon-melody","version":1,"notes":[{"startMs":1000,"durMs":500,"midi":60}]}"#;

    fn env(ref_status: Option<&str>, duration_ms: Option<i64>) -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        conn.execute(
            "INSERT INTO songs (id, path, file_name, title, extension, size_bytes, modified_ms, duration_ms, added_ms, updated_ms)
             VALUES ('s1', 'C:/v/s1.mp4', 's1.mp4', 's1', 'mp4', 1, 1, ?1, 1, 1)",
            [duration_ms],
        )
        .unwrap();
        if let Some(st) = ref_status {
            conn.execute(
                "INSERT INTO melody_references (song_id, source, format_version, status, offset_ms, notes_json, created_ms, updated_ms)
                 VALUES ('s1', 'json', 1, ?1, 120, ?2, 1, 1)",
                (st, REF),
            )
            .unwrap();
        }
        // Passed leak check for the default device at full volume (competitive gate).
        let mut conn = conn;
        set(&mut conn, check(None, 1.0, true));
        (dir, conn)
    }

    fn set(conn: &mut Connection, v: serde_json::Value) {
        settings::write(conn, v.as_object().cloned().unwrap()).unwrap();
    }

    fn check(device: Option<&str>, volume: f64, passed: bool) -> serde_json::Value {
        serde_json::json!({"leakCheck": {"deviceId": device, "videoVolume": volume, "passed": passed, "atMs": 1}})
    }

    #[test]
    fn leak_check_gates_competitive() {
        let reason_leak =
            "Faça o teste de vazamento do microfone (ou use fones) para pontuar no ranking.";
        let competitive = |conn: &Connection| prepare(conn, &args()).unwrap().2;
        let (_d, mut conn) = env(Some("validated"), Some(10_000));

        set(&mut conn, check(None, 0.6, true));
        set(&mut conn, serde_json::json!({"videoVolume": 0.65}));
        assert!(competitive(&conn).competitive, "volume within +0.05");
        set(&mut conn, serde_json::json!({"videoVolume": 0.7}));
        let s = competitive(&conn);
        assert!(!s.competitive, "louder than checked");
        assert_eq!(s.reason.as_deref(), Some(reason_leak));

        set(&mut conn, serde_json::json!({"videoVolume": 0.5}));
        set(&mut conn, check(None, 0.6, false));
        assert!(!competitive(&conn).competitive, "failed check");
        set(&mut conn, check(Some("USB Mic"), 0.6, true));
        assert!(!competitive(&conn).competitive, "checked another device");
        set(&mut conn, serde_json::json!({"inputDeviceId": "USB Mic"}));
        assert!(competitive(&conn).competitive, "same device");

        // Never checked → training.
        conn.execute("DELETE FROM settings WHERE key = 'leakCheck'", [])
            .unwrap();
        assert!(!competitive(&conn).competitive);
    }

    #[test]
    fn leak_check_run_saves_verdict_only() {
        let (_d, mut conn) = env(Some("experimental"), Some(10_000));
        set(
            &mut conn,
            serde_json::json!({"videoVolume": 0.8, "inputDeviceId": "Mic"}),
        );
        let run = StartPerformance {
            leak_check: true,
            ..args()
        };
        let (cfg, ctx, started) = prepare(&conn, &run).unwrap();
        assert!(cfg.leak_check && !cfg.competitive && !started.competitive);

        let verdict = |passed: bool| {
            let mut s = summary(false, 9_000, Some(3.0));
            s.leak = Some(LeakVerdict {
                passed,
                guide_score: if passed { 0.01 } else { 0.4 },
                reason: (!passed).then(|| "vazou".to_owned()),
            });
            s
        };
        let out = persist(&mut conn, &ctx, verdict(true)).unwrap();
        assert_eq!(
            (out.performance_id, out.result, out.competitive),
            (None, None, false)
        );
        assert!(out.leak.as_ref().is_some_and(|l| l.passed));
        let saved = settings::load(&conn).unwrap().leak_check.unwrap();
        assert_eq!(
            (saved.device_id.as_deref(), saved.video_volume, saved.passed),
            (Some("Mic"), 0.8, true)
        );
        let n: i64 = conn
            .query_row("SELECT count(*) FROM performances", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "leak runs never reach performances");

        // A later failed check replaces the pass.
        let out = persist(&mut conn, &ctx, verdict(false)).unwrap();
        assert_eq!(out.leak.and_then(|l| l.reason).as_deref(), Some("vazou"));
        assert!(!settings::load(&conn).unwrap().leak_check.unwrap().passed);

        // Not inside a round / for a participant, and needs a usable reference.
        let ana = sessions::create_participant(&conn, "Ana").unwrap().id;
        let in_round = StartPerformance {
            participant_id: Some(ana),
            ..run.clone()
        };
        assert_eq!(prepare_err(&conn, &in_round), "invalid_input");
        for status in [None, Some("invalid")] {
            let (_d, conn) = env(status, Some(10_000));
            assert_eq!(prepare_err(&conn, &run), "invalid_input");
        }
    }

    #[test]
    fn latency_probe_saves_offset_only_when_passed() {
        let (_d, mut conn) = env(None, Some(10_000));
        set(&mut conn, serde_json::json!({"latencyOffsetMs": 120}));
        let probe = StartPerformance {
            latency_probe: Some(vec![1000.0, 2000.0, 3000.0]),
            ..args()
        };
        let (cfg, ctx, started) = prepare(&conn, &probe).unwrap();
        assert_eq!(cfg.latency_offset_ms, 0.0, "probe measures raw latency");
        assert_eq!(
            cfg.latency_probe.as_deref(),
            Some(&[1000.0, 2000.0, 3000.0][..])
        );
        assert!(!cfg.leak_check && !started.competitive && ctx.latency_probe);

        let verdict = |passed: bool, offset_ms: f32| {
            let mut s = summary(false, 3_000, None);
            s.exploratory = None;
            s.latency = Some(LatencyVerdict {
                result: audio_engine::LatencyResult {
                    offset_ms,
                    jitter_ms: 4.0,
                    detected: 3,
                    expected: 3,
                },
                passed,
                reason: (!passed).then(|| "palmas fora do tempo".to_owned()),
            });
            s
        };
        let out = persist(&mut conn, &ctx, verdict(false, 300.0)).unwrap();
        assert!(out.latency.is_some_and(|l| !l.passed));
        assert_eq!(
            settings::load(&conn).unwrap().latency_offset_ms,
            120,
            "failed probe keeps the setting"
        );
        let out = persist(&mut conn, &ctx, verdict(true, 187.6)).unwrap();
        assert_eq!((out.performance_id, out.competitive), (None, false));
        assert_eq!(settings::load(&conn).unwrap().latency_offset_ms, 188);
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(
            json["latency"]["offsetMs"], 187.6_f32 as f64,
            "flattened as in ipc.ts"
        );
        let n: i64 = conn
            .query_row("SELECT count(*) FROM performances", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);

        for bad in [
            StartPerformance {
                leak_check: true,
                ..probe.clone()
            },
            StartPerformance {
                latency_probe: Some(vec![]),
                ..args()
            },
            StartPerformance {
                latency_probe: Some(vec![1.0; 501]),
                ..args()
            },
        ] {
            assert_eq!(prepare_err(&conn, &bad), "invalid_input");
        }
    }

    #[test]
    fn reference_capture_stores_notes_as_new_active_reference() {
        // Works over an existing reference and without one.
        let (_d, mut conn) = env(Some("validated"), Some(10_000));
        let old = reference::active(&conn, "s1").unwrap().unwrap();
        let run = StartPerformance {
            reference_capture: true,
            ..args()
        };
        let (cfg, ctx, started) = prepare(&conn, &run).unwrap();
        assert!(cfg.reference_capture && cfg.reference.is_none() && !cfg.competitive);
        assert!(!started.competitive && started.reason.is_some());
        assert_eq!(ctx.reference, None, "nothing scored against the old one");

        let sung = |n: u32| {
            let mut s = summary(false, 9_000, None);
            s.captured_notes = Some(
                (0..n)
                    .map(|i| scoring_engine::Note {
                        start_ms: 1_000 + i * 600,
                        dur_ms: 500,
                        midi: 60.0 + i as f32,
                        lyric: None,
                    })
                    .collect(),
            );
            s
        };
        // Too few notes: nothing stored, the old reference stays active.
        let out = persist(&mut conn, &ctx, sung(2)).unwrap();
        assert_eq!(out.captured_reference, None);
        assert_eq!(out.reason.as_deref(), Some(REASON_FEW_NOTES));
        assert_eq!(reference::active(&conn, "s1").unwrap().unwrap().id, old.id);
        let out = persist(&mut conn, &ctx, summary(false, 9_000, None)).unwrap();
        assert_eq!(out.reason.as_deref(), Some(REASON_FEW_NOTES), "no notes");

        let out = persist(&mut conn, &ctx, sung(3)).unwrap();
        let new = out.captured_reference.clone().unwrap();
        assert_eq!(
            (
                new.source.as_str(),
                new.status.as_str(),
                new.revision,
                new.offset_ms
            ),
            ("manual", "experimental", 1, 0)
        );
        assert_eq!(new.notes.len(), 3);
        assert_eq!((new.notes[2].start_ms, new.notes[2].midi), (2_200, 62.0));
        assert_eq!(reference::active(&conn, "s1").unwrap(), Some(new));
        assert_eq!(
            (out.performance_id, out.competitive, out.reason.as_deref()),
            (None, false, None)
        );
        let (refs, perfs): (i64, i64) = conn
            .query_row(
                "SELECT (SELECT count(*) FROM melody_references), (SELECT count(*) FROM performances)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((refs, perfs), (2, 0), "old reference kept for history");
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["capturedReference"]["source"], "manual");

        // Song without a reference.
        let (_d, mut conn) = env(None, None);
        let (_, ctx, _) = prepare(&conn, &run).unwrap();
        persist(&mut conn, &ctx, sung(4)).unwrap();
        assert_eq!(
            reference::active(&conn, "s1").unwrap().unwrap().notes.len(),
            4
        );

        // Runs alone.
        let ana = sessions::create_participant(&conn, "Ana").unwrap().id;
        for bad in [
            StartPerformance {
                leak_check: true,
                ..run.clone()
            },
            StartPerformance {
                latency_probe: Some(vec![1.0]),
                ..run.clone()
            },
            StartPerformance {
                participant_id: Some(ana),
                ..run.clone()
            },
        ] {
            assert_eq!(prepare_err(&conn, &bad), "invalid_input");
        }
    }

    #[test]
    fn leak_check_is_read_only_for_update_settings() {
        let (_d, mut conn) = env(None, None);
        let patch = check(None, 1.0, true).as_object().cloned().unwrap();
        assert_eq!(
            settings::update(&mut conn, patch).unwrap_err().code,
            "invalid_setting"
        );
    }

    fn prepare_err(conn: &Connection, a: &StartPerformance) -> &'static str {
        match prepare(conn, a) {
            Err(e) => e.code,
            Ok(_) => panic!("expected an error"),
        }
    }

    fn args() -> StartPerformance {
        StartPerformance {
            song_id: "s1".into(),
            round_id: None,
            participant_id: None,
            leak_check: false,
            latency_probe: None,
            reference_capture: false,
        }
    }

    fn summary(competitive: bool, played_ms: u64, score: Option<f32>) -> PerformanceSummary {
        PerformanceSummary {
            competitive,
            reason: None,
            played_ms,
            result: score.map(|score| ScoreResult {
                score,
                pitch: 0.9,
                rhythm: 0.9,
                sustain: 0.9,
                coverage: 1.0,
                notes_total: 1,
                notes_hit: 1,
                algorithm_version: scoring_engine::ALGORITHM_VERSION,
            }),
            exploratory: score.is_none().then_some(ExploratoryMetrics {
                voiced_ratio: 0.5,
                pitch_stability: 0.8,
            }),
            leak: None,
            latency: None,
            captured_notes: None,
            capture: CaptureStats {
                device_id: None,
                sample_rate: 48_000,
                noise_floor_dbfs: -60.0,
                gate_margin_db: 10.0,
                latency_offset_ms: 0.0,
                clipped_ratio: 0.0,
                leak_suspected: false,
                missing_ratio: 0.0,
            },
        }
    }

    #[test]
    fn prepare_reports_competitiveness() {
        let (_d, conn) = env(Some("validated"), Some(10_000));
        let (cfg, ctx, started) = prepare(&conn, &args()).unwrap();
        assert!(started.competitive && cfg.competitive && started.reason.is_none());
        assert_eq!(
            cfg.reference.as_ref().map(|(r, o)| (r.notes.len(), *o)),
            Some((1, 120))
        );
        assert_eq!(ctx.reference.map(|(_, rev)| rev), Some(1));

        for (status, competitive) in [
            (Some("experimental"), false),
            (Some("invalid"), false),
            (None, false),
        ] {
            let (_d, conn) = env(status, Some(10_000));
            let (cfg, _, started) = prepare(&conn, &args()).unwrap();
            assert_eq!(started.competitive, competitive);
            assert!(started.reason.is_some());
            assert_eq!(cfg.reference.is_some(), status.is_some());
        }
        let (_d, conn) = env(None, None);
        let bad = StartPerformance {
            song_id: "nope".into(),
            ..args()
        };
        assert_eq!(prepare_err(&conn, &bad), "not_found");
        let bad = StartPerformance {
            round_id: Some(9),
            ..args()
        };
        assert_eq!(prepare_err(&conn, &bad), "not_found");
    }

    #[test]
    fn competitive_needs_half_the_song() {
        let (_d, mut conn) = env(Some("validated"), Some(10_000));
        let (_, ctx, _) = prepare(&conn, &args()).unwrap();
        let short = persist(&mut conn, &ctx, summary(true, 4_999, Some(70.0))).unwrap();
        assert!(!short.competitive, "below 50% → training");
        assert!(short.performance_id.is_some(), "training is stored too");
        assert_eq!(short.song_record, None, "training does not set records");
        let full = persist(&mut conn, &ctx, summary(true, 5_000, Some(80.0))).unwrap();
        assert!(full.competitive);
        assert_eq!(full.song_record, Some(80.0));
        let seeked = persist(&mut conn, &ctx, summary(false, 10_000, Some(95.0))).unwrap();
        assert!(!seeked.competitive);
        assert_eq!(seeked.song_record, Some(80.0));
        let n: i64 = conn
            .query_row("SELECT count(*) FROM performances", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 3);
        let stored: String = conn
            .query_row("SELECT capture_json FROM performances LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(stored.contains("\"noiseFloorDbfs\""));
    }

    #[test]
    fn training_runs_always_explain_why() {
        let (_d, mut conn) = env(Some("validated"), Some(10_000));
        let (_, ctx, _) = prepare(&conn, &args()).unwrap();
        // Competitive run: no reason.
        let ok = persist(&mut conn, &ctx, summary(true, 9_000, Some(80.0))).unwrap();
        assert!(ok.competitive);
        assert_eq!(ok.reason, None);
        // The session's own reason (e.g. a seek) is passed through.
        let mut seek = summary(false, 9_000, Some(80.0));
        seek.reason = Some("Houve um salto no vídeo.".into());
        let out = persist(&mut conn, &ctx, seek).unwrap();
        assert_eq!(out.reason.as_deref(), Some("Houve um salto no vídeo."));
        // This module's 50% gate.
        let short = persist(&mut conn, &ctx, summary(true, 4_000, Some(80.0))).unwrap();
        assert!(!short.competitive);
        assert_eq!(short.reason.as_deref(), Some(REASON_HALF));

        // Reason known at start wins (reference not validated).
        let (_d, mut conn) = env(Some("experimental"), Some(10_000));
        let (_, ctx, started) = prepare(&conn, &args()).unwrap();
        let out = persist(&mut conn, &ctx, summary(false, 9_000, Some(80.0))).unwrap();
        assert_eq!(out.reason, started.reason);
        // Unknown duration.
        let (_d, mut conn) = env(Some("validated"), None);
        let (_, ctx, _) = prepare(&conn, &args()).unwrap();
        let out = persist(&mut conn, &ctx, summary(true, 9_000, Some(80.0))).unwrap();
        assert_eq!(out.reason.as_deref(), Some(REASON_NO_DURATION));
        // capture_json carries whatever CaptureStats serializes (missingRatio included).
        let capture: serde_json::Value = serde_json::from_str(
            &conn
                .query_row("SELECT capture_json FROM performances LIMIT 1", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
        )
        .unwrap();
        assert!(capture.get("missingRatio").is_some());
    }

    #[test]
    fn unknown_duration_is_training_and_no_reference_stores_nothing() {
        let (_d, mut conn) = env(Some("validated"), None);
        let (_, ctx, _) = prepare(&conn, &args()).unwrap();
        assert!(
            !persist(&mut conn, &ctx, summary(true, 60_000, Some(90.0)))
                .unwrap()
                .competitive
        );

        let (_d, mut conn) = env(None, Some(10_000));
        let (_, ctx, _) = prepare(&conn, &args()).unwrap();
        let out = persist(&mut conn, &ctx, summary(false, 10_000, None)).unwrap();
        assert_eq!(
            (out.performance_id, out.result, out.competitive),
            (None, None, false)
        );
        assert!(out.exploratory.is_some());
        let n: i64 = conn
            .query_row("SELECT count(*) FROM performances", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn round_completes_and_records_per_participant() {
        let (_d, mut conn) = env(Some("validated"), Some(10_000));
        let a = sessions::create_participant(&conn, "Ana").unwrap().id;
        let b = sessions::create_participant(&conn, "Bia").unwrap().id;
        let s = sessions::create_session(
            &mut conn,
            serde_json::from_value(serde_json::json!({
                "mode": "duel", "name": null, "participantIds": [a, b], "songIds": ["s1"], "order": "manual"
            }))
            .unwrap(),
        )
        .unwrap();
        let (ra, rb) = (&s.rounds[0], &s.rounds[1]);
        let wrong = StartPerformance {
            song_id: "s1".into(),
            round_id: Some(ra.id),
            participant_id: Some(b),
            leak_check: false,
            latency_probe: None,
            reference_capture: false,
        };
        assert_eq!(prepare_err(&conn, &wrong), "invalid_input");

        let start = |round: &sessions::Round| StartPerformance {
            song_id: "s1".into(),
            round_id: Some(round.id),
            participant_id: round.participant_id,
            leak_check: false,
            latency_probe: None,
            reference_capture: false,
        };
        let (_, ctx_a, _) = prepare(&conn, &start(ra)).unwrap();
        // Training run: round stays pending.
        persist(&mut conn, &ctx_a, summary(true, 1_000, Some(99.0))).unwrap();
        assert_eq!(
            sessions::get_session(&conn, s.id).unwrap().rounds[0].status,
            "pending"
        );
        let out = persist(&mut conn, &ctx_a, summary(true, 9_000, Some(70.0))).unwrap();
        assert_eq!(
            (out.personal_best, out.song_record),
            (Some(70.0), Some(70.0))
        );
        let (_, ctx_b, _) = prepare(&conn, &start(rb)).unwrap();
        let out = persist(&mut conn, &ctx_b, summary(true, 9_000, Some(60.0))).unwrap();
        assert_eq!(
            (out.personal_best, out.song_record),
            (Some(60.0), Some(70.0))
        );

        let s = sessions::get_session(&conn, s.id).unwrap();
        assert!(s.rounds.iter().all(|r| r.status == "completed"));
        assert_eq!(s.standings[0].participant_id, a);
        assert_eq!(
            prepare_err(&conn, &start(ra)),
            "invalid_input",
            "completed round"
        );
    }
}
