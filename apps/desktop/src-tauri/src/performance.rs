//! Live performance session (SPEC-006 T-006-03). Owner: Agent 2; registered in
//! `lib.rs` by Agent 1. Sync model: docs/architecture/overview.md.
//!
//! No database access here: the commands load the reference and settings,
//! decide whether the reference allows a competitive run, forward ticks to the
//! UI channel and persist the [`PerformanceSummary`].
//!
//! Clocks: the `<video>` is the music clock, the microphone sample counter is
//! the voice clock. `sync()` anchors `(video_time, callback_sample_count)`; a
//! frame's song time is
//! `anchor_video + (frame_centre_sample − anchor_sample) / sr − latency_offset`.

use crate::error::AppError;
use audio_engine::{
    AnalysisFrame, Capture, CaptureConfig, LatencyResult, detect_onsets, latency_from, window_len,
};
use scoring_engine::{
    Difficulty, ExploratoryMetrics, LiveFeedback, MelodyReference, ScoreResult, Scorer, VoiceFrame,
    exploratory_metrics,
};
use serde::Serialize;
use std::sync::{Arc, Mutex, MutexGuard};

/// Re-anchor when the estimated song time drifts more than this from the video.
const RESYNC_MS: f64 = 40.0;
/// One UI tick every N analysis frames (10 ms each) ≈ 33 Hz. The scorer gets every frame.
const TICK_EVERY: u64 = 3;
/// Leak heuristic (SPEC-004 R9, indicative only): voiced frames farther than
/// this from any note count as "outside notes".
const LEAK_MARGIN_MS: f64 = 300.0;
const LEAK_MIN_FRAMES: u32 = 100;
const LEAK_RATIO: f32 = 0.2;
/// Leak check (T-004-06, SPEC-004 R9): the user stays silent while the song
/// plays; whatever pitch credit the scorer finds came from the speakers.
/// Threshold and evidence: SPEC-004 R9 / `leak_threshold_evidence` below.
const LEAK_MAX_GUIDE_SCORE: f32 = 0.05;
const LEAK_MIN_PLAYED_MS: u64 = 8000;
const LEAK_MIN_NOTES: u32 = 3;
/// Latency probe (T-004-07, SPEC-004 R7): minimum clicks played and valid range.
const PROBE_MIN_CLICKS: usize = 4;
const PROBE_MAX_OFFSET_MS: f32 = 500.0;
/// SPEC-006 v3 N2: above this share of missing mic audio the run is training.
pub const MAX_MISSING_RATIO: f32 = 0.05;

#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Active reference and its offset (ms). `None` = exploratory metrics only.
    pub reference: Option<(MelodyReference, i32)>,
    /// The reference is `validated` (the command decides). A seek or a missing
    /// reference makes the run non-competitive regardless.
    pub competitive: bool,
    pub difficulty: Difficulty,
    /// Microphone input + video output latency (settings, SPEC-004 R7).
    pub latency_offset_ms: f64,
    pub device_id: Option<String>,
    pub capture: CaptureConfig,
    /// Leak check run (user silent): never competitive, yields `PerformanceSummary::leak`.
    pub leak_check: bool,
    /// Latency probe: click times in VIDEO time (ms) of the click track the UI
    /// plays. Exclusive with `leak_check`; requires `latency_offset_ms == 0`
    /// (otherwise it would measure the residual). No analysis, no `LiveTick`.
    pub latency_probe: Option<Vec<f64>>,
}

/// Outcome of a latency probe run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyVerdict {
    /// `offsetMs` = median(onset song time − click time); goes to `latencyOffsetMs`.
    #[serde(flatten)]
    pub result: LatencyResult,
    pub passed: bool,
    /// pt-BR, why it did not pass; `None` when passed.
    pub reason: Option<String>,
}

/// Outcome of a leak check run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeakVerdict {
    pub passed: bool,
    /// Pitch component P over the reference notes fully played (0..1).
    pub guide_score: f32,
    /// pt-BR, why it did not pass; `None` when passed.
    pub reason: Option<String>,
}

/// Meter data for the UI; also used by the mic monitor command.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicFrame {
    pub rms_dbfs: f32,
    pub f0_hz: Option<f32>,
    pub confidence: f32,
    pub voiced: bool,
    pub clipped: bool,
}

impl From<&AnalysisFrame> for MicFrame {
    fn from(f: &AnalysisFrame) -> Self {
        Self {
            rms_dbfs: f.rms_dbfs,
            f0_hz: f.f0_hz,
            confidence: f.confidence,
            voiced: f.voiced,
            clipped: f.clipped,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTick {
    pub mic: MicFrame,
    /// Latest scorer feedback; `None` outside notes, while paused or without reference.
    pub feedback: Option<LiveFeedback>,
}

/// Goes to `performances.capture_json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStats {
    pub device_id: Option<String>,
    pub sample_rate: u32,
    pub noise_floor_dbfs: f32,
    pub gate_margin_db: f32,
    pub latency_offset_ms: f64,
    /// Over the frames that were scored (music playing).
    pub clipped_ratio: f32,
    /// Indicative only (the singer may hum in instrumentals); never invalidates a run.
    pub leak_suspected: bool,
    /// 0..1: microphone audio that never arrived while the music played
    /// (1 − callback samples / (played video time · sample rate)). Above
    /// [`MAX_MISSING_RATIO`] the run is training (SPEC-006 v3 N2).
    pub missing_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSummary {
    /// Validated reference, a result, no seek and ≤ 5% missing audio. The
    /// command still requires `played_ms ≥ 50%` of the song (SPEC-007 R2).
    pub competitive: bool,
    /// pt-BR: why a run that started competitive ended as training (seek or
    /// missing audio). `None` otherwise; the start reason covers the rest.
    pub reason: Option<String>,
    /// Song time actually played (playing = true), skipped stretches excluded.
    pub played_ms: u64,
    pub result: Option<ScoreResult>,
    /// Set when there is no reference.
    pub exploratory: Option<ExploratoryMetrics>,
    pub capture: CaptureStats,
    /// Set for leak check runs only.
    pub leak: Option<LeakVerdict>,
    /// Set for latency probe runs only.
    pub latency: Option<LatencyVerdict>,
}

/// Running performance: microphone capture + session state. Drop = abort.
pub struct LivePerformance {
    capture: Capture,
    session: Arc<Mutex<Session>>,
}

fn lock(s: &Mutex<Session>) -> MutexGuard<'_, Session> {
    // A panic while holding the lock cannot leave the session half-updated in a
    // way worse than losing the run; keep going with the inner value.
    s.lock().unwrap_or_else(|e| e.into_inner())
}

impl LivePerformance {
    /// Opens the microphone. Frames are discarded until the first
    /// `sync(.., playing = true, ..)`. `on_tick` runs on the analysis thread.
    pub fn start(
        cfg: PerformanceConfig,
        mut on_tick: impl FnMut(LiveTick) + Send + 'static,
    ) -> Result<Self, AppError> {
        validate(&cfg)?;
        let session = Arc::new(Mutex::new(Session::new(&cfg)));
        let shared = session.clone();
        let device = cfg.device_id.as_deref();
        let capture = if cfg.latency_probe.is_some() {
            Capture::start_raw(device, move |i, block| lock(&shared).on_samples(i, block))
        } else {
            Capture::start(device, cfg.capture, move |f| {
                let tick = lock(&shared).on_frame(&f);
                if let Some(t) = tick {
                    on_tick(t);
                }
            })
        }
        .map_err(|e| AppError::new("mic_unavailable", e.to_string()))?;
        lock(&session).set_sample_rate(capture.sample_rate());
        Ok(Self { capture, session })
    }

    /// Call on play, pause, seeking/seeked and every ~250 ms with the video's
    /// media time. Non-finite or negative times are ignored.
    pub fn sync(&self, video_time_s: f64, playing: bool, seeked: bool) {
        let now = self.capture.captured_samples();
        lock(&self.session).sync(video_time_s, playing, seeked, now);
    }

    /// Driver error (e.g. microphone unplugged); the run stops receiving frames.
    pub fn take_error(&self) -> Option<AppError> {
        self.capture
            .take_error()
            .map(|e| AppError::new("mic_error", e.to_string()))
    }

    pub fn finish(self) -> PerformanceSummary {
        let now = self.capture.captured_samples();
        let Self { capture, session } = self;
        drop(capture); // joins the analysis thread: no frame arrives after this
        lock(&session).summary(now)
    }
}

fn validate(cfg: &PerformanceConfig) -> Result<(), AppError> {
    let Some(clicks) = &cfg.latency_probe else {
        return Ok(());
    };
    let bad = |m: &str| Err(AppError::new("invalid_input", m));
    if cfg.leak_check {
        return bad("A sonda de latência e a checagem de vazamento não podem rodar juntas.");
    }
    if cfg.latency_offset_ms != 0.0 {
        return bad("A sonda de latência precisa rodar com o ajuste de latência em 0.");
    }
    if clicks.iter().any(|t| !t.is_finite() || *t < 0.0) {
        return bad("Os instantes dos cliques da sonda são inválidos.");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Anchor {
    video_ms: f64,
    sample: u64,
}

/// Hardware-free session logic (unit-tested below).
struct Session {
    scorer: Option<Scorer>,
    /// Note spans in song time, for the leak heuristic.
    spans: Vec<(f64, f64)>,
    /// Frames kept only without a reference (exploratory metrics).
    voice: Vec<VoiceFrame>,
    competitive: bool,
    latency_ms: f64,
    device_id: Option<String>,
    capture_cfg: CaptureConfig,
    sample_rate: u32,
    half_window: u64,
    /// `None` = paused / not started: frames are discarded.
    anchor: Option<Anchor>,
    /// Frames centred before this callback sample predate the current play
    /// segment (captured while paused or before a seek).
    discard_before: u64,
    last_video_ms: f64,
    /// Measured on the video clock: the mic sample clock can lose samples
    /// (WASAPI delivered 40–46 k/s at a nominal 48 k on the dev machine).
    played_ms: f64,
    /// Callback samples delivered over the same played stretches (N2).
    played_samples: u64,
    last_sync_sample: u64,
    /// Competitive at start: a later loss of competitiveness gets a reason.
    started_competitive: bool,
    frames_seen: u64,
    scored_frames: u32,
    clipped_frames: u32,
    outside_frames: u32,
    outside_voiced: u32,
    feedback: Option<LiveFeedback>,
    leak_check: bool,
    /// Video time where playback first started (leak check coverage).
    play_from: Option<f64>,
    seeked: bool,
    /// Latency probe: click times (video ms), raw samples since the first play
    /// and every anchor used, so each onset maps with the anchor of its time.
    probe: Option<Vec<f64>>,
    probe_samples: Vec<f32>,
    probe_first: u64,
    anchors: Vec<Anchor>,
}

impl Session {
    fn new(cfg: &PerformanceConfig) -> Self {
        let (scorer, spans) = match &cfg.reference {
            Some((r, offset)) => (
                Some(Scorer::new(r, *offset, cfg.difficulty)),
                r.notes
                    .iter()
                    .map(|n| {
                        let s = n.start_ms as f64 + *offset as f64;
                        (s, s + n.dur_ms as f64)
                    })
                    .collect(),
            ),
            None => (None, Vec::new()),
        };
        let competitive =
            cfg.competitive && scorer.is_some() && !cfg.leak_check && cfg.latency_probe.is_none();
        Self {
            competitive,
            started_competitive: competitive,
            played_samples: 0,
            last_sync_sample: 0,
            scorer,
            spans,
            voice: Vec::new(),
            latency_ms: cfg.latency_offset_ms,
            device_id: cfg.device_id.clone(),
            capture_cfg: cfg.capture,
            sample_rate: 0,
            half_window: 0,
            anchor: None,
            discard_before: 0,
            last_video_ms: 0.0,
            played_ms: 0.0,
            frames_seen: 0,
            scored_frames: 0,
            clipped_frames: 0,
            outside_frames: 0,
            outside_voiced: 0,
            feedback: None,
            leak_check: cfg.leak_check,
            play_from: None,
            seeked: false,
            probe: cfg.latency_probe.clone().map(|mut c| {
                c.sort_by(f64::total_cmp);
                c
            }),
            probe_samples: Vec::new(),
            probe_first: 0,
            anchors: Vec::new(),
        }
    }

    fn set_sample_rate(&mut self, sr: u32) {
        self.sample_rate = sr;
        self.half_window = window_len(sr) as u64 / 2;
    }

    fn samples_to_ms(&self, samples: f64) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            samples * 1000.0 / self.sample_rate as f64
        }
    }

    /// Song (video) time at callback sample `n`, extrapolated from the anchor.
    fn video_ms_at(&self, a: Anchor, n: u64) -> f64 {
        a.video_ms + self.samples_to_ms(n as f64 - a.sample as f64)
    }

    fn close_segment(&mut self) {
        self.anchor = None;
    }

    fn sync(&mut self, video_s: f64, playing: bool, seeked: bool, now: u64) {
        if !video_s.is_finite() || video_s < 0.0 {
            return;
        }
        let video_ms = video_s * 1000.0;
        let position = match self.anchor {
            Some(a) => self.video_ms_at(a, now),
            None => self.last_video_ms,
        };
        if self.anchor.is_some() {
            let reached = if seeked { position } else { video_ms };
            self.played_ms += (reached - self.last_video_ms).max(0.0);
            self.played_samples += now.saturating_sub(self.last_sync_sample);
        }
        self.last_video_ms = video_ms;
        self.last_sync_sample = now;
        if seeked {
            // Seek = training (overview §6). Notes jumped over are ignored, not zeroed.
            // ponytail: a backward seek re-sings notes on top of their first pass;
            // fine for training, revisit if seeks ever count.
            self.competitive = false;
            self.seeked = true;
            if video_ms > position
                && let Some(s) = &mut self.scorer
            {
                s.skip(position, video_ms);
            }
            self.close_segment();
        }
        if !playing {
            self.close_segment();
            return;
        }
        match self.anchor {
            Some(a) if (self.video_ms_at(a, now) - video_ms).abs() <= RESYNC_MS => {}
            Some(_) => {
                // Drift: re-anchor without reprocessing; played time stays continuous.
                self.close_segment();
                self.anchor = Some(Anchor {
                    video_ms,
                    sample: now,
                });
            }
            None => {
                self.anchor = Some(Anchor {
                    video_ms,
                    sample: now,
                });
                self.discard_before = now;
                self.play_from.get_or_insert(video_ms);
            }
        }
        if let Some(a) = self.anchor
            && self.anchors.last() != Some(&a)
        {
            self.anchors.push(a);
        }
    }

    /// Latency probe: keeps raw samples from the first play on (no pre-roll).
    fn on_samples(&mut self, first: u64, block: &[f32]) {
        let Some(start) = self.anchors.first().map(|a| a.sample) else {
            return;
        };
        if self.probe.is_none() {
            return;
        }
        let skip = start.saturating_sub(first).min(block.len() as u64) as usize;
        let block = &block[skip..];
        if block.is_empty() {
            return;
        }
        let first = first + skip as u64;
        if self.probe_samples.is_empty() {
            self.probe_first = first;
        }
        // A gap in the numbering (never expected) is filled with silence to keep
        // sample positions exact.
        let expected = self.probe_first + self.probe_samples.len() as u64;
        if first > expected {
            self.probe_samples
                .resize(self.probe_samples.len() + (first - expected) as usize, 0.0);
        }
        self.probe_samples.extend_from_slice(block);
    }

    fn latency_verdict(&self, end_ms: f64) -> LatencyVerdict {
        let from = self.play_from.unwrap_or(f64::INFINITY);
        let clicks: Vec<f64> = self
            .probe
            .iter()
            .flatten()
            .copied()
            .filter(|&c| c >= from && c <= end_ms)
            .collect();
        let sr = self.sample_rate as f64;
        let mut onsets: Vec<f64> = if sr > 0.0 {
            detect_onsets(
                &self.probe_samples,
                self.sample_rate,
                self.capture_cfg.noise_floor_dbfs,
            )
            .into_iter()
            .filter_map(|ms| {
                let n = self.probe_first as f64 + ms * sr / 1000.0;
                let a = self.anchors.iter().rev().find(|a| a.sample as f64 <= n)?;
                Some(a.video_ms + (n - a.sample as f64) * 1000.0 / sr - self.latency_ms)
            })
            .collect()
        } else {
            Vec::new()
        };
        onsets.sort_by(f64::total_cmp);
        let result = latency_from(&onsets, &clicks);
        let reason = if self.seeked {
            Some("A sonda foi interrompida: deixe os cliques tocarem sem avançar ou voltar.")
        } else if clicks.len() < PROBE_MIN_CLICKS {
            Some("Sonda curta demais: deixe todos os cliques tocarem.")
        } else if result.detected * 2 < result.expected {
            Some(
                "Não detectamos os cliques ou as palmas. Aumente o volume ou bata palmas mais forte.",
            )
        } else if result.offset_ms.abs() > PROBE_MAX_OFFSET_MS {
            Some("A latência medida ficou fora da faixa de −500 a +500 ms. Repita a sonda.")
        } else {
            None
        };
        LatencyVerdict {
            result,
            passed: reason.is_none(),
            reason: reason.map(str::to_owned),
        }
    }

    fn on_frame(&mut self, f: &AnalysisFrame) -> Option<LiveTick> {
        self.frames_seen += 1;
        let centre = f.sample_index + self.half_window;
        match self.anchor {
            Some(a) if centre >= self.discard_before => {
                let t_ms = self.video_ms_at(a, centre) - self.latency_ms;
                self.score(f, t_ms);
            }
            _ => self.feedback = None,
        }
        self.frames_seen
            .is_multiple_of(TICK_EVERY)
            .then(|| LiveTick {
                mic: MicFrame::from(f),
                feedback: self.feedback,
            })
    }

    fn score(&mut self, f: &AnalysisFrame, t_ms: f64) {
        self.scored_frames += 1;
        self.clipped_frames += u32::from(f.clipped);
        let near_note = self
            .spans
            .iter()
            .any(|&(s, e)| t_ms >= s - LEAK_MARGIN_MS && t_ms < e + LEAK_MARGIN_MS);
        if !self.spans.is_empty() && !near_note {
            self.outside_frames += 1;
            self.outside_voiced += u32::from(f.voiced);
        }
        let vf = VoiceFrame {
            t_ms,
            f0_hz: f.f0_hz,
            confidence: f.confidence,
            voiced: f.voiced,
        };
        match &mut self.scorer {
            Some(s) => self.feedback = s.push(vf),
            None => self.voice.push(vf),
        }
    }

    fn summary(&mut self, now: u64) -> PerformanceSummary {
        let end_ms = match self.anchor {
            Some(a) => self.video_ms_at(a, now),
            None => self.last_video_ms,
        };
        if self.anchor.is_some() {
            // Tail since the last sync, estimated on the mic clock (≤ one sync period).
            self.played_ms += (end_ms - self.last_video_ms).max(0.0);
            self.played_samples += now.saturating_sub(self.last_sync_sample);
        }
        self.close_segment();
        let mut covered = 0u32;
        if self.leak_check
            && let Some(scorer) = &mut self.scorer
        {
            // Judge only the notes that were played in full.
            let from = self.play_from.unwrap_or(f64::INFINITY);
            for &(s, e) in &self.spans {
                if s >= from && e <= end_ms {
                    covered += 1;
                } else {
                    scorer.skip(s, s + 0.5);
                }
            }
        }
        let result = self.scorer.take().map(Scorer::finish);
        let leak = self
            .leak_check
            .then(|| self.leak_verdict(result.as_ref(), covered));
        let latency = self.probe.is_some().then(|| self.latency_verdict(end_ms));
        let ratio = |n: u32, d: u32| if d == 0 { 0.0 } else { n as f32 / d as f32 };
        let missing_ratio = if self.played_ms > 0.0 {
            (1.0 - self.samples_to_ms(self.played_samples as f64) / self.played_ms).max(0.0) as f32
        } else {
            0.0
        };
        let gappy = missing_ratio > MAX_MISSING_RATIO;
        let reason = if !self.started_competitive {
            None
        } else if self.seeked {
            Some(
                "A música foi avançada ou voltada durante a performance. Esta performance conta como treino."
                    .to_owned(),
            )
        } else if gappy {
            let pct = format!("{:.1}", missing_ratio * 100.0).replace('.', ",");
            Some(format!(
                "O áudio do microfone teve falhas ({pct}% do áudio não chegou). Esta performance conta como treino."
            ))
        } else {
            None
        };
        PerformanceSummary {
            competitive: self.competitive && result.is_some() && !gappy,
            reason,
            played_ms: self.played_ms.round() as u64,
            exploratory: (result.is_none() && self.probe.is_none())
                .then(|| exploratory_metrics(&self.voice)),
            result,
            capture: CaptureStats {
                device_id: self.device_id.clone(),
                sample_rate: self.sample_rate,
                noise_floor_dbfs: self.capture_cfg.noise_floor_dbfs,
                gate_margin_db: self.capture_cfg.gate_margin_db,
                latency_offset_ms: self.latency_ms,
                clipped_ratio: ratio(self.clipped_frames, self.scored_frames),
                leak_suspected: self.outside_frames >= LEAK_MIN_FRAMES
                    && ratio(self.outside_voiced, self.outside_frames) > LEAK_RATIO,
                missing_ratio,
            },
            leak,
            latency,
        }
    }

    fn leak_verdict(&self, result: Option<&ScoreResult>, covered: u32) -> LeakVerdict {
        let guide_score = result.map_or(0.0, |r| r.pitch);
        let played = self.played_ms.round() as u64;
        let reason = if result.is_none() {
            Some("O teste precisa de uma música com referência melódica.")
        } else if self.seeked {
            Some("O teste foi interrompido: deixe a música tocar sem avançar ou voltar.")
        } else if played < LEAK_MIN_PLAYED_MS || covered < LEAK_MIN_NOTES {
            Some("Teste curto demais: deixe tocar pelo menos 8 segundos com 3 notas da melodia.")
        } else if guide_score > LEAK_MAX_GUIDE_SCORE {
            Some(
                "O microfone está captando a voz ou a melodia da música. Use fones de ouvido ou abaixe o volume.",
            )
        } else {
            None
        };
        LeakVerdict {
            passed: reason.is_none(),
            guide_score,
            reason: reason.map(str::to_owned),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoring_engine::Note;

    const SR: u32 = 48_000;
    const HOP: u64 = 480;
    const HALF: f64 = 1024.0; // window_len(48 kHz) / 2

    fn midi_hz(m: f32) -> f32 {
        440.0 * 2f32.powf((m - 69.0) / 12.0)
    }

    fn frame(sample_index: u64, f0: Option<f32>) -> AnalysisFrame {
        AnalysisFrame {
            sample_index,
            sample_rate: SR,
            f0_hz: f0,
            confidence: if f0.is_some() { 0.95 } else { 0.1 },
            rms_dbfs: if f0.is_some() { -20.0 } else { -80.0 },
            voiced: f0.is_some(),
            clipped: false,
        }
    }

    fn session(notes: Option<&[(u32, u32, f32)]>, competitive: bool, latency: f64) -> Session {
        let reference = notes.map(|ns| {
            let r = MelodyReference {
                format_version: 1,
                notes: ns
                    .iter()
                    .map(|&(s, d, m)| Note {
                        start_ms: s,
                        dur_ms: d,
                        midi: m,
                        lyric: None,
                    })
                    .collect(),
            };
            (r, 0)
        });
        let mut s = Session::new(&PerformanceConfig {
            reference,
            competitive,
            difficulty: Difficulty::Normal,
            latency_offset_ms: latency,
            device_id: Some("mic".into()),
            capture: CaptureConfig::default(),
            leak_check: false,
            latency_probe: None,
        });
        s.set_sample_rate(SR);
        s
    }

    /// Feeds every hop-spaced frame whose window starts in `[from, to)` samples.
    fn feed(s: &mut Session, from: u64, to: u64, f0: Option<f32>) -> Vec<LiveTick> {
        (from.div_ceil(HOP)..to.div_ceil(HOP))
            .filter_map(|k| s.on_frame(&frame(k * HOP, f0)))
            .collect()
    }

    #[test]
    fn frame_time_uses_window_centre_anchor_and_latency() {
        let mut s = session(None, false, 100.0);
        feed(&mut s, 0, 48_000, Some(220.0)); // before any sync: discarded
        assert!(s.voice.is_empty());
        s.sync(10.0, true, false, 48_000);
        feed(&mut s, 46_080, 49_000, Some(220.0)); // centres before the anchor are discarded
        // Windows start at 46 080 + k·480; centre = start + 1024 must be ≥ 48 000:
        // 47 040, 47 520, 48 000, 48 480, 48 960.
        assert_eq!(s.voice.len(), 5);
        let t0 = s.voice[0].t_ms;
        let expected = 10_000.0 + (47_040.0 + HALF - 48_000.0) * 1000.0 / SR as f64 - 100.0;
        assert!((t0 - expected).abs() < 1e-9, "{t0} vs {expected}");
    }

    #[test]
    fn pause_discards_frames_and_stops_the_played_clock() {
        let mut s = session(None, false, 0.0);
        s.sync(0.0, true, false, 0);
        feed(&mut s, 0, 48_000, Some(220.0));
        let before = s.voice.len();
        s.sync(1.0, false, false, 48_000); // pause after 1 s
        feed(&mut s, 48_000, 96_000, Some(220.0));
        assert_eq!(s.voice.len(), before);
        s.sync(1.0, true, false, 96_000); // resume after 1 s paused
        feed(&mut s, 96_000, 144_000, Some(220.0));
        let first_after = s.voice[before].t_ms;
        assert!((first_after - (1000.0 + HALF * 1000.0 / SR as f64)).abs() < 1e-9);
        let sum = s.summary(144_000);
        assert_eq!(sum.played_ms, 2000);
        let ex = sum.exploratory.unwrap();
        assert!(ex.voiced_ratio == 1.0 && ex.pitch_stability > 0.99);
    }

    #[test]
    fn small_drift_keeps_the_anchor_large_drift_reanchors() {
        let mut s = session(None, false, 0.0);
        s.sync(0.0, true, false, 0);
        s.sync(1.03, true, false, 48_000); // 30 ms off: keep
        assert_eq!(s.anchor.map(|a| a.sample), Some(0));
        s.sync(1.1, true, false, 48_000); // 100 ms off: re-anchor
        let a = s.anchor.unwrap();
        assert_eq!((a.video_ms, a.sample), (1100.0, 48_000));
        // Played time follows the video clock: 1.1 s by the last sync + 1 s tail.
        assert_eq!(s.summary(96_000).played_ms, 2100);
    }

    #[test]
    fn seek_makes_training_and_ignores_jumped_notes() {
        // Notes start after a short lead-in (the first analysis frame is centred
        // ~21 ms after capture starts; v2 counts frames that cannot exist as missing).
        let notes = [(100, 400, 60.0), (1000, 500, 62.0), (5100, 400, 64.0)];
        let mut s = session(Some(&notes), true, 0.0);
        s.sync(0.0, true, false, 0);
        feed(&mut s, 0, 28_800, Some(midi_hz(60.0))); // sing 0–0.6 s
        s.sync(5.0, true, true, 28_800); // seek 0.6 s → 5 s
        feed(&mut s, 28_800, 57_600, Some(midi_hz(64.0))); // sing 5–5.6 s
        let sum = s.summary(57_600);
        let r = sum.result.unwrap();
        assert!(!sum.competitive);
        assert!(
            sum.reason
                .is_some_and(|m| m.contains("avançada ou voltada"))
        );
        assert_eq!((r.notes_total, r.notes_hit), (2, 2));
        assert!(r.score >= 99.0, "{r:?}");
        assert_eq!(sum.played_ms, 1200);
    }

    #[test]
    fn missing_mic_audio_over_5_percent_makes_training_with_a_reason() {
        // SPEC-006 v3 N2: 2 s of video; the callback delivers `per_s` samples per second.
        let run = |per_s: u64, competitive: bool| {
            let mut s = session(Some(&[(0, 500, 60.0)]), competitive, 0.0);
            s.sync(0.0, true, false, 0);
            s.sync(1.0, true, false, per_s);
            s.sync(2.0, false, false, 2 * per_s); // pause: closes the played stretch
            s.sync(2.0, true, false, 3 * per_s); // resume: the paused second is not counted
            s.summary(3 * per_s)
        };
        let ok = run(46_000, true); // 4.2 % missing
        assert!(
            (ok.capture.missing_ratio - 1.0 / 24.0).abs() < 1e-4,
            "{ok:?}"
        );
        assert!(ok.competitive && ok.reason.is_none(), "{ok:?}");
        let bad = run(44_000, true); // 8.3 % missing
        assert!(!bad.competitive);
        assert_eq!(
            bad.reason.as_deref(),
            Some(
                "O áudio do microfone teve falhas (8,3% do áudio não chegou). Esta performance conta como treino."
            )
        );
        // A run that was never competitive keeps the start reason (none here).
        let training = run(44_000, false);
        assert!(!training.competitive && training.reason.is_none());
        assert!(
            serde_json::to_string(&bad.capture)
                .unwrap()
                .contains("\"missingRatio\":0.083")
        );
    }

    #[test]
    fn unseeked_validated_run_stays_competitive() {
        let notes = [(100, 500, 60.0)];
        let mut s = session(Some(&notes), true, 0.0);
        s.sync(0.0, true, false, 0);
        feed(&mut s, 0, 48_000, Some(midi_hz(60.0)));
        let sum = s.summary(48_000);
        assert!(sum.competitive);
        assert!(sum.exploratory.is_none());
        assert_eq!(sum.result.unwrap().notes_hit, 1);
    }

    #[test]
    fn ticks_are_decimated_and_carry_feedback_only_while_playing() {
        let notes = [(0, 5000, 60.0)];
        let mut s = session(Some(&notes), false, 0.0);
        let paused = feed(&mut s, 0, 9 * HOP, Some(midi_hz(60.0)));
        assert_eq!(paused.len(), 3);
        assert!(paused.iter().all(|t| t.feedback.is_none()));
        s.sync(0.0, true, false, 9 * HOP);
        let playing = feed(&mut s, 9 * HOP, 99 * HOP, Some(midi_hz(60.0)));
        assert_eq!(playing.len(), 30);
        assert!(playing.last().unwrap().feedback.is_some());
    }

    #[test]
    fn singing_far_from_notes_flags_a_possible_leak_without_invalidating() {
        let notes = [(5000, 1000, 60.0)];
        let mut s = session(Some(&notes), true, 0.0);
        s.sync(0.0, true, false, 0);
        feed(&mut s, 0, 3 * 48_000, Some(220.0)); // 3 s of "voice" in the intro
        let sum = s.summary(3 * 48_000);
        assert!(sum.capture.leak_suspected);
        assert!(sum.competitive);
    }

    #[test]
    fn no_reference_is_never_competitive_and_bad_times_are_ignored() {
        let mut s = session(None, true, 0.0);
        s.sync(f64::NAN, true, false, 0);
        s.sync(-1.0, true, false, 0);
        assert!(s.anchor.is_none());
        let sum = s.summary(0);
        assert!(!sum.competitive && sum.result.is_none() && sum.exploratory.is_some());
    }

    #[test]
    #[ignore = "needs an audio input device"]
    fn live_run_on_the_default_microphone() {
        let ticks = Arc::new(Mutex::new(0u32));
        let counter = ticks.clone();
        let cfg = PerformanceConfig {
            reference: None,
            competitive: true,
            difficulty: Difficulty::Normal,
            latency_offset_ms: 0.0,
            device_id: None,
            capture: CaptureConfig::default(),
            leak_check: false,
            latency_probe: None,
        };
        let p = LivePerformance::start(cfg, move |_| {
            *counter.lock().unwrap_or_else(|e| e.into_inner()) += 1
        })
        .expect("default microphone opens");
        std::thread::sleep(std::time::Duration::from_millis(1000)); // stream start-up
        p.sync(0.0, true, false);
        std::thread::sleep(std::time::Duration::from_millis(1500));
        p.sync(1.5, true, false);
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(p.take_error().is_none());
        let sum = p.finish();
        let ticks = *ticks.lock().unwrap();
        eprintln!(
            "played {} ms, {ticks} ticks, {:?}",
            sum.played_ms, sum.capture
        );
        // Video clock up to the last sync (1.5 s) + mic-clock tail (~0.5 s, which can
        // run short when the driver drops samples).
        // The 1.5 s up to the last sync is exact; the tail is a lower-bounded estimate.
        assert!((1500..=2200).contains(&sum.played_ms), "{}", sum.played_ms);
        // ~33 Hz nominal; follows the samples the driver actually delivers, which
        // varied 40–46 k/s here, so only require that ticks flow (≥ 10 Hz).
        assert!(ticks >= 30, "{ticks} ticks in ~3 s");
        assert!(!sum.competitive && sum.exploratory.is_some());
    }

    #[test]
    fn summary_serializes_for_capture_json() {
        let json = serde_json::to_string(&session(None, false, 12.5).summary(0)).unwrap();
        assert!(json.contains("\"playedMs\":0"));
        assert!(json.contains("\"leakSuspected\":false"));
        assert!(json.contains("\"latencyOffsetMs\":12.5"));
    }

    // ---------- Leak check (T-004-06) ----------

    /// Regression song (scoring-engine): 10 × 1000 ms notes, 200 ms rests, from 500 ms.
    fn leak_song() -> Vec<(u32, u32, f32)> {
        [60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 67.0, 65.0, 64.0, 62.0]
            .iter()
            .enumerate()
            .map(|(i, &m)| (500 + i as u32 * 1200, 1000, m))
            .collect()
    }

    fn leak_session(notes: &[(u32, u32, f32)], leak_check: bool) -> Session {
        let mut s = session(Some(notes), true, 0.0);
        s.leak_check = leak_check;
        s.competitive = !leak_check;
        s
    }

    #[derive(Clone, Copy, Debug)]
    enum Source {
        /// Sung vowel /a/ (formants F1 700, F2 1220, F3 2600 Hz) on the melody.
        GuideVocal,
        /// Piano-like melody line: 1/k² harmonics, 300 ms decay per note.
        MelodyInstrument,
        /// Accompaniment without melody: root and fifth one octave below, held.
        ChordsOnly,
    }

    /// Speaker sound reaching the mic at `rms_dbfs` (RMS while sounding; −120 =
    /// none) plus room noise at −60 dBFS RMS. Deterministic.
    fn speakers(notes: &[(u32, u32, f32)], src: Source, rms_dbfs: f64) -> Vec<f32> {
        use std::f64::consts::TAU;
        let end_ms = notes.last().map_or(0, |n| n.0 + n.1) as f64 + 700.0;
        let n = (end_ms / 1000.0 * SR as f64) as usize;
        let hz = |m: f64| 440.0 * 2f64.powf((m - 69.0) / 12.0);
        let formant = |f: f64| {
            [
                (700.0, 8.0, 130.0),
                (1220.0, 6.0, 150.0),
                (2600.0, 3.0, 250.0),
            ]
            .iter()
            .map(|&(fc, a, bw)| a * (-((f - fc) / bw).powi(2)).exp())
            .sum::<f64>()
                + 1.0
        };
        let mut raw = vec![0.0f64; n];
        let silent = rms_dbfs <= -100.0;
        for (i, x) in raw.iter_mut().enumerate().filter(|_| !silent) {
            let t = i as f64 * 1000.0 / SR as f64;
            let Some(&(start, dur, m)) = notes.iter().rev().find(|nt| t >= nt.0 as f64) else {
                continue;
            };
            let (m, since) = (m as f64, t - start as f64);
            let in_note = since < dur as f64;
            let ts = i as f64 / SR as f64;
            *x = match src {
                Source::GuideVocal if in_note => (1..=20)
                    .map(|k| {
                        let f = hz(m) * k as f64;
                        formant(f) / k as f64 * (TAU * f * ts).sin()
                    })
                    .sum(),
                Source::MelodyInstrument if in_note => {
                    (1..=8)
                        .map(|k| (TAU * hz(m) * k as f64 * ts).sin() / (k * k) as f64)
                        .sum::<f64>()
                        * (-since / 300.0).exp()
                }
                Source::ChordsOnly => [m - 12.0, m - 5.0]
                    .iter()
                    .map(|&c| (TAU * hz(c) * ts).sin())
                    .sum(),
                _ => 0.0,
            };
        }
        let sounding: Vec<f64> = raw.iter().copied().filter(|v| *v != 0.0).collect();
        let rms =
            (sounding.iter().map(|v| v * v).sum::<f64>() / sounding.len().max(1) as f64).sqrt();
        let gain = if rms > 0.0 {
            10f64.powf(rms_dbfs / 20.0) / rms
        } else {
            0.0
        };
        let noise_amp = 10f64.powf(-60.0 / 20.0) * 3f64.sqrt(); // uniform: RMS = amp/√3
        let mut seed = 0x5EED_u64;
        raw.iter()
            .map(|v| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let u = 2.0 * ((seed >> 11) as f64 / (1u64 << 53) as f64) - 1.0;
                (v * gain + noise_amp * u) as f32
            })
            .collect()
    }

    /// Runs the audio through the real analysis and a session from video time 0.
    fn run_audio(mut s: Session, audio: &[f32]) -> PerformanceSummary {
        s.sync(0.0, true, false, 0);
        for f in audio_engine::analyze_buffer(audio, SR, &CaptureConfig::default()) {
            s.on_frame(&f);
        }
        s.summary(audio.len() as u64)
    }

    /// Evidence for LEAK_MAX_GUIDE_SCORE (SPEC-004 R9). User silent; only the
    /// speakers reach the mic (gate: default noise floor −60 + 10 dB). For each
    /// case: the leak check verdict and the score the leak alone would earn in a
    /// normal run. Invariant: a passed check never lets the leak earn > 5 points.
    /// ~60 s in a debug build (audio synthesis), so manual:
    /// `cargo test -p master-of-notes leak_threshold_evidence -- --ignored --nocapture`
    #[test]
    #[ignore = "slow evidence sweep; numbers are in SPEC-004 R9"]
    fn leak_threshold_evidence() {
        let notes = leak_song();
        eprintln!("| Fonte | Nível (dBFS RMS) | guide_score | Passou | Score só com o vazamento |");
        eprintln!("|---|---|---|---|---|");
        let mut rows = Vec::new();
        for src in [
            Source::GuideVocal,
            Source::MelodyInstrument,
            Source::ChordsOnly,
        ] {
            for level in [-20.0, -30.0, -40.0, -45.0, -50.0, -55.0, -120.0] {
                let audio = speakers(&notes, src, level);
                let check = run_audio(leak_session(&notes, true), &audio);
                let normal = run_audio(leak_session(&notes, false), &audio);
                let v = check.leak.clone().expect("leak verdict");
                let leak_score = normal.result.map_or(0.0, |r| r.score);
                eprintln!(
                    "| {src:?} | {level} | {:.3} | {} | {leak_score:.1} |",
                    v.guide_score, v.passed
                );
                rows.push((src, level, v, leak_score));
            }
        }
        for (src, level, v, leak_score) in &rows {
            if v.passed {
                assert!(
                    *leak_score <= 5.0,
                    "{src:?} at {level} passed but leaks {leak_score}"
                );
            }
        }
        // Room noise alone (no speaker sound) must pass.
        assert!(rows.iter().filter(|r| r.1 == -120.0).all(|r| r.2.passed));
        // A guide vocal well above the gate must fail.
        assert!(
            rows.iter()
                .any(|r| matches!(r.0, Source::GuideVocal) && r.1 == -20.0 && !r.2.passed)
        );
    }

    #[test]
    fn leak_check_needs_8s_and_3_notes_and_no_seek() {
        let notes = leak_song();
        let audio = speakers(&notes, Source::GuideVocal, -120.0);
        // Full run with no speaker leak: passes, never competitive.
        let ok = run_audio(leak_session(&notes, true), &audio);
        assert!(ok.leak.as_ref().is_some_and(|v| v.passed), "{:?}", ok.leak);
        assert!(!ok.competitive);
        // 5 s only: too short.
        let short = run_audio(leak_session(&notes, true), &audio[..5 * SR as usize]);
        let v = short.leak.unwrap();
        assert!(!v.passed && v.reason.unwrap().contains("curto"));
        // Seek during the check: invalid.
        let mut s = leak_session(&notes, true);
        s.sync(0.0, true, false, 0);
        s.sync(6.0, true, true, 48_000);
        let v = s.summary(20 * 48_000).leak.unwrap();
        assert!(!v.passed && v.reason.unwrap().contains("interrompido"));
        // No reference: cannot judge.
        let mut no_ref = session(None, false, 0.0);
        no_ref.leak_check = true;
        let v = no_ref.summary(0).leak.unwrap();
        assert!(!v.passed && v.reason.unwrap().contains("referência"));
        // Normal runs have no verdict.
        assert!(
            run_audio(leak_session(&notes, false), &audio)
                .leak
                .is_none()
        );
    }

    #[test]
    fn leak_threshold_separates_a_silent_room_from_a_leak() {
        // Fast guard (ideal frames): the guide melody reaching the mic is caught;
        // a few stray in-tune frames (40 ms per 1000 ms note) are tolerated.
        let notes = leak_song();
        let run = |voiced_ms: u32| {
            let mut s = leak_session(&notes, true);
            s.sync(0.0, true, false, 0);
            for k in 0..(13_000 * 48 / HOP) {
                let centre_ms = (k as f64 * HOP as f64 + HALF) / 48.0;
                let f0 = notes
                    .iter()
                    .find(|n| centre_ms >= n.0 as f64 && centre_ms < (n.0 + voiced_ms) as f64)
                    .map(|n| midi_hz(n.2));
                s.on_frame(&frame(k * HOP, f0));
            }
            s.summary(13_000 * 48).leak.unwrap()
        };
        let leak = run(1000);
        assert!(!leak.passed && leak.guide_score > 0.99, "{leak:?}");
        let stray = run(40);
        assert!(
            stray.passed && stray.guide_score <= LEAK_MAX_GUIDE_SCORE,
            "{stray:?}"
        );
    }

    // ---------- Latency probe (T-004-07) ----------

    fn probe_clicks() -> Vec<f64> {
        (0..8).map(|i| 500.0 + i as f64 * 750.0).collect()
    }

    fn probe_session(clicks: Vec<f64>) -> Session {
        let mut s = session(None, false, 0.0);
        s.probe = Some(clicks);
        s
    }

    /// Mic signal of `len` samples: room noise (−60 dBFS) + clap-like bursts at
    /// the given sample positions.
    fn mic_with_claps(len: usize, claps: &[usize]) -> Vec<f32> {
        let mut seed = 0xC1AF_u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (2.0 * ((seed >> 11) as f64 / (1u64 << 53) as f64) - 1.0) as f32
        };
        let room = 10f32.powf(-60.0 / 20.0) * 3f32.sqrt();
        let mut x: Vec<f32> = (0..len).map(|_| room * rnd()).collect();
        for &c in claps {
            for k in 0..1440 {
                if let Some(s) = x.get_mut(c + k) {
                    *s += 0.3 * rnd() * (-(k as f32) / 288.0).exp();
                }
            }
        }
        x
    }

    fn feed_raw(s: &mut Session, x: &[f32]) {
        for (i, block) in x.chunks(480).enumerate() {
            s.on_samples(i as u64 * 480, block);
        }
    }

    #[test]
    fn probe_measures_the_delay_through_the_anchor_and_skips_the_pre_roll() {
        // Playback starts (video 0) when the callback counter is at 24 000; the
        // mic hears each click 87 ms later. A loud bang in the pre-roll is ignored.
        let clicks = probe_clicks();
        let start = 24_000usize;
        let mut claps: Vec<usize> = clicks
            .iter()
            .map(|c| start + ((c + 87.0) * 48.0) as usize)
            .collect();
        claps.push(2_000);
        let x = mic_with_claps(start + 7 * 48_000, &claps);
        let mut s = probe_session(clicks);
        s.sync(0.0, true, false, start as u64);
        feed_raw(&mut s, &x);
        let sum = s.summary(x.len() as u64);
        let v = sum.latency.unwrap();
        assert!(v.passed, "{v:?}");
        assert_eq!((v.result.detected, v.result.expected), (8, 8));
        assert!((v.result.offset_ms - 87.0).abs() <= 1.0, "{v:?}");
        assert!(!sum.competitive && sum.result.is_none() && sum.exploratory.is_none());
        assert_eq!(s.probe_first, start as u64); // pre-roll not kept
    }

    #[test]
    fn probe_maps_each_onset_with_the_anchor_of_its_time() {
        // At 3 s of mic time the video reports 3.1 s (re-anchor): clicks after it
        // are measured against the new anchor (+100 ms).
        let clicks: Vec<f64> = (0..5).map(|i| 3500.0 + i as f64 * 750.0).collect();
        let claps: Vec<usize> = clicks
            .iter()
            .map(|c| ((c + 60.0) * 48.0) as usize)
            .collect();
        let x = mic_with_claps(8 * 48_000, &claps);
        let mut s = probe_session(clicks);
        s.sync(0.0, true, false, 0);
        s.sync(3.1, true, false, 3 * 48_000);
        feed_raw(&mut s, &x);
        let v = s.summary(x.len() as u64).latency.unwrap();
        assert!(v.passed, "{v:?}");
        assert!((v.result.offset_ms - 160.0).abs() <= 1.0, "{v:?}");
    }

    #[test]
    fn probe_fails_without_detections_or_after_a_seek() {
        let x = mic_with_claps(7 * 48_000, &[]);
        let mut s = probe_session(probe_clicks());
        s.sync(0.0, true, false, 0);
        feed_raw(&mut s, &x);
        let v = s.summary(x.len() as u64).latency.unwrap();
        assert!(!v.passed && v.result.detected == 0);
        assert!(v.reason.unwrap().contains("Não detectamos"));

        let mut s = probe_session(probe_clicks());
        s.sync(0.0, true, false, 0);
        s.sync(4.0, true, true, 48_000);
        let v = s.summary(7 * 48_000).latency.unwrap();
        assert!(!v.passed && v.reason.unwrap().contains("interrompida"));

        let mut s = probe_session(probe_clicks());
        s.sync(0.0, true, false, 0);
        let v = s.summary(48_000).latency.unwrap(); // stopped after 1 s: 1 click
        assert!(!v.passed && v.reason.unwrap().contains("curta"));
    }

    #[test]
    fn probe_config_is_validated_and_serializes_flat() {
        let base = PerformanceConfig {
            reference: None,
            competitive: false,
            difficulty: Difficulty::Normal,
            latency_offset_ms: 0.0,
            device_id: None,
            capture: CaptureConfig::default(),
            leak_check: false,
            latency_probe: Some(probe_clicks()),
        };
        assert!(validate(&base).is_ok());
        let both = PerformanceConfig {
            leak_check: true,
            ..base.clone()
        };
        assert!(validate(&both).is_err());
        let offset = PerformanceConfig {
            latency_offset_ms: 120.0,
            ..base.clone()
        };
        assert!(validate(&offset).is_err());
        let nan = PerformanceConfig {
            latency_probe: Some(vec![f64::NAN]),
            ..base.clone()
        };
        assert!(validate(&nan).is_err());
        assert!(
            validate(&PerformanceConfig {
                latency_probe: None,
                ..base
            })
            .is_ok()
        );

        let v = LatencyVerdict {
            result: LatencyResult {
                offset_ms: 87.0,
                jitter_ms: 1.0,
                detected: 8,
                expected: 8,
            },
            passed: true,
            reason: None,
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(
            json.contains("\"offsetMs\":87.0") && json.contains("\"passed\":true"),
            "{json}"
        );
        assert!(!json.contains("\"result\""), "{json}");
    }

    #[test]
    #[ignore = "needs an audio input device"]
    fn live_probe_on_the_default_microphone() {
        // No click track is played here: proves the raw capture path end to end
        // (clicks counted as expected, a verdict with a reason, no ticks).
        let cfg = PerformanceConfig {
            reference: None,
            competitive: false,
            difficulty: Difficulty::Normal,
            latency_offset_ms: 0.0,
            device_id: None,
            capture: CaptureConfig::default(),
            leak_check: false,
            latency_probe: Some(vec![300.0, 800.0, 1300.0, 1800.0]),
        };
        let ticks = Arc::new(Mutex::new(0u32));
        let counter = ticks.clone();
        let p = LivePerformance::start(cfg, move |_| {
            *counter.lock().unwrap_or_else(|e| e.into_inner()) += 1
        })
        .expect("default microphone opens");
        std::thread::sleep(std::time::Duration::from_millis(800));
        p.sync(0.0, true, false);
        std::thread::sleep(std::time::Duration::from_millis(2300));
        let sum = p.finish();
        let v = sum.latency.expect("probe verdict");
        eprintln!("played {} ms, {v:?}", sum.played_ms);
        assert_eq!(v.result.expected, 4);
        assert!(v.passed || v.reason.is_some());
        assert_eq!(*ticks.lock().unwrap(), 0);
    }

    // ---------- Performance measurement (T-006-07, docs/testing/performance.md) ----------
    // Manual, `--ignored`: they take minutes and the live ones need a microphone.
    // Process stats come from the OS (Get-Process), the same numbers Task Manager shows.

    #[derive(Debug, Clone, Copy)]
    struct ProcStats {
        cpu_s: f64,
        working_set_mb: f64,
        private_mb: f64,
        threads: u32,
    }

    fn proc_stats() -> ProcStats {
        let script = format!(
            "$p=Get-Process -Id {}; [string]::Format([cultureinfo]::InvariantCulture,'{{0}} {{1}} {{2}} {{3}}',$p.TotalProcessorTime.TotalSeconds,$p.WorkingSet64,$p.PrivateMemorySize64,$p.Threads.Count)",
            std::process::id()
        );
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .expect("powershell runs");
        let text = String::from_utf8_lossy(&out.stdout);
        let v: Vec<f64> = text
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        assert_eq!(v.len(), 4, "unexpected Get-Process output: {text}");
        ProcStats {
            cpu_s: v[0],
            working_set_mb: v[1] / 1_048_576.0,
            private_mb: v[2] / 1_048_576.0,
            threads: v[3] as u32,
        }
    }

    /// 5-minute reference: 500 notes of 500 ms with 100 ms rests.
    fn five_minute_song() -> Vec<(u32, u32, f32)> {
        (0..500)
            .map(|i| (500 + i * 600, 500, 60.0 + (i % 12) as f32))
            .collect()
    }

    fn song_config(probe: bool) -> PerformanceConfig {
        let notes = five_minute_song();
        PerformanceConfig {
            reference: Some((
                MelodyReference {
                    format_version: 1,
                    notes: notes
                        .iter()
                        .map(|&(s, d, m)| Note {
                            start_ms: s,
                            dur_ms: d,
                            midi: m,
                            lyric: None,
                        })
                        .collect(),
                },
                0,
            )),
            competitive: true,
            difficulty: Difficulty::Normal,
            latency_offset_ms: 0.0,
            device_id: None,
            capture: CaptureConfig::default(),
            leak_check: false,
            latency_probe: probe.then(Vec::new),
        }
    }

    fn percentile(sorted: &[f64], p: f64) -> f64 {
        if sorted.is_empty() {
            return f64::NAN;
        }
        sorted[((sorted.len() - 1) as f64 * p).round() as usize]
    }

    /// (a) compute cost and (d) finish cost, offline: 5 min of a sung melody
    /// (4-harmonic tone on the reference) through the real analysis and session.
    #[test]
    #[ignore = "measurement (T-006-07), ~10-60 s"]
    fn perf_offline_5_minutes() {
        use std::time::Instant;
        let notes = five_minute_song();
        let secs = 300.5;
        let n = (secs * SR as f64) as usize;
        let mut phase = 0.0f64;
        let audio: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 * 1000.0 / SR as f64;
                match notes
                    .iter()
                    .find(|x| t >= x.0 as f64 && t < (x.0 + x.1) as f64)
                {
                    Some(x) => {
                        let hz = 440.0 * 2f64.powf((x.2 as f64 - 69.0) / 12.0);
                        phase = (phase + std::f64::consts::TAU * hz / SR as f64)
                            % std::f64::consts::TAU;
                        (0.2 * (1..=4)
                            .map(|k| (phase * k as f64).sin() / k as f64)
                            .sum::<f64>()) as f32
                    }
                    None => 0.0,
                }
            })
            .collect();
        let before = proc_stats();
        let t0 = Instant::now();
        let frames = audio_engine::analyze_buffer(&audio, SR, &CaptureConfig::default());
        let t_analysis = t0.elapsed().as_secs_f64();
        let mut s = Session::new(&song_config(false));
        s.set_sample_rate(SR);
        s.sync(0.0, true, false, 0);
        let t1 = Instant::now();
        for f in &frames {
            s.on_frame(f);
        }
        let t_session = t1.elapsed().as_secs_f64();
        let t2 = Instant::now();
        let sum = s.summary(n as u64);
        let t_finish = t2.elapsed().as_secs_f64();
        let after = proc_stats();
        let r = sum.result.unwrap();
        eprintln!(
            "PERF offline {secs} s audio, {} frames: analysis {:.2} s ({:.2}% of a core), session+scorer {:.3} s ({:.3}%), finish {:.3} ms; score {} ({}/{} notes); private MB {:.0} -> {:.0}",
            frames.len(),
            t_analysis,
            100.0 * t_analysis / secs,
            t_session,
            100.0 * t_session / secs,
            1000.0 * t_finish,
            r.score,
            r.notes_hit,
            r.notes_total,
            before.private_mb,
            after.private_mb
        );
        assert_eq!(r.notes_total, 500);
        assert!(r.score >= 95.0, "{r:?}");
    }

    /// (a) live CPU/memory over 5 min and (b) latency from the audio callback
    /// to on_tick, on the default microphone, in real time.
    #[test]
    #[ignore = "measurement (T-006-07), 5 min, needs a microphone"]
    fn perf_live_5_minutes_cpu_memory_and_tick_latency() {
        use std::time::{Duration, Instant};
        let minutes: f64 = std::env::var("PERF_MINUTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5.0);
        if std::env::var("PERF_HIGH_PRIORITY").is_ok() {
            // Profiling experiment: is the latency tail OS scheduling under load?
            let script = format!(
                "(Get-Process -Id {}).PriorityClass='High'",
                std::process::id()
            );
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &script])
                .status();
        }
        let session = Arc::new(Mutex::new(Session::new(&song_config(false))));
        let ticks = Arc::new(Mutex::new(Vec::<(Instant, u64)>::with_capacity(40_000)));
        let (s2, t2) = (session.clone(), ticks.clone());
        let before = proc_stats();
        let half = window_len(SR) as u64 / 2;
        let cap = Capture::start(None, CaptureConfig::default(), move |f| {
            let tick = lock(&s2).on_frame(&f);
            if tick.is_some() {
                // The newest audio this tick depends on: the frame's last sample.
                let end = f.sample_index + 2 * half;
                t2.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push((Instant::now(), end));
            }
        })
        .expect("default microphone opens");
        lock(&session).set_sample_rate(cap.sample_rate());
        std::thread::sleep(Duration::from_millis(500));
        let start = Instant::now();
        lock(&session).sync(0.0, true, false, cap.captured_samples());
        // Mid-run process stats on another thread: never stall the polling loop.
        let mid_stats = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f64(minutes * 30.0));
            proc_stats()
        });
        // Poll the callback counter. Each change is stored with the last poll that
        // still saw the old value and the first that sees the new one: the callback
        // delivered those samples in between. ~1 change per 10 ms driver period.
        let mut arrivals: Vec<(Instant, Instant, u64)> = Vec::with_capacity(40_000);
        let mut next_sync = Duration::from_millis(250);
        let (mut last_poll, mut last_n) = (Instant::now(), cap.captured_samples());
        while start.elapsed().as_secs_f64() < minutes * 60.0 {
            let now = Instant::now();
            let n = cap.captured_samples();
            if n != last_n {
                arrivals.push((last_poll, now, n));
                last_n = n;
            }
            last_poll = now;
            if start.elapsed() >= next_sync {
                // UI-like sync every 250 ms with the "video" time = wall time.
                lock(&session).sync(start.elapsed().as_secs_f64(), true, false, n);
                next_sync += Duration::from_millis(250);
            }
            std::thread::sleep(Duration::from_micros(500));
        }
        let mid = mid_stats.join().ok();
        let end_stats = proc_stats();
        let err = cap.take_error();
        let captured_end = cap.captured_samples();
        drop(cap);
        let t_fin = Instant::now();
        let sum = lock(&session).summary(captured_end);
        let finish_ms = t_fin.elapsed().as_secs_f64() * 1000.0;
        let ticks = ticks.lock().unwrap_or_else(|e| e.into_inner());
        // Callback arrival of sample `end`: midpoint of the poll bracket of the
        // first counter change reaching it. Brackets > 5 ms (polling stalled) are
        // too uncertain and are counted apart, not used.
        let mut lat: Vec<f64> = Vec::with_capacity(ticks.len());
        let mut worst: Vec<(f64, f64)> = Vec::new();
        let (mut j, mut uncertain) = (0, 0u32);
        for &(t_tick, end) in ticks.iter() {
            while j < arrivals.len() && arrivals[j].2 < end {
                j += 1;
            }
            if j == 0 || j >= arrivals.len() {
                continue;
            }
            let (t0, t1, _) = arrivals[j];
            if t1 - t0 > Duration::from_millis(5) {
                uncertain += 1;
                continue;
            }
            let t_cb = t0 + (t1 - t0) / 2;
            if t_tick >= t_cb {
                let ms = (t_tick - t_cb).as_secs_f64() * 1000.0;
                lat.push(ms);
                worst.push((ms, (t_tick - start).as_secs_f64()));
            }
        }
        lat.sort_by(f64::total_cmp);
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        eprintln!(
            "PERF worst (ms, s since start): {:?}",
            &worst[..worst.len().min(6)]
        );
        // Driver delivery period: time between consecutive counter changes.
        let mut gaps: Vec<f64> = arrivals
            .windows(2)
            .map(|w| (w[1].1 - w[0].1).as_secs_f64() * 1000.0)
            .collect();
        gaps.sort_by(f64::total_cmp);
        let wall = start.elapsed().as_secs_f64();
        let cpu = end_stats.cpu_s - before.cpu_s;
        eprintln!(
            "PERF live {wall:.0} s: process CPU {cpu:.2} s = {:.2}% of a core (includes the 0.5 ms polling loop); \
             private MB {:.1} -> mid {:.1} -> end {:.1}; WS MB {:.1} -> {:.1}; threads {} -> {}; ticks {} ({:.1}/s); \
             latency callback->on_tick ms p50 {:.1} p95 {:.1} p99 {:.1} max {:.1} (n {}, {uncertain} uncertain dropped); driver period ms p50 {:.2} p99 {:.2}; \
             finish {finish_ms:.2} ms; err {err:?}; played {} ms",
            100.0 * cpu / wall,
            before.private_mb,
            mid.map_or(f64::NAN, |m| m.private_mb),
            end_stats.private_mb,
            before.working_set_mb,
            end_stats.working_set_mb,
            before.threads,
            end_stats.threads,
            ticks.len(),
            ticks.len() as f64 / wall,
            percentile(&lat, 0.5),
            percentile(&lat, 0.95),
            percentile(&lat, 0.99),
            lat.last().copied().unwrap_or(f64::NAN),
            lat.len(),
            percentile(&gaps, 0.5),
            percentile(&gaps, 0.99),
            sum.played_ms,
        );
        assert!(err.is_none());
        assert!(!lat.is_empty());
    }

    /// (c) stability: 10 performances in a row in the same process; threads and
    /// memory must return to the same level.
    #[test]
    #[ignore = "measurement (T-006-07), ~10 × 30 s, needs a microphone"]
    fn perf_stability_10_runs() {
        use std::time::{Duration, Instant};
        let secs: u64 = std::env::var("PERF_RUN_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let baseline = proc_stats();
        let mut rows = Vec::new();
        for run in 1..=10 {
            let p = LivePerformance::start(song_config(false), |_| {}).expect("mic opens");
            let start = Instant::now();
            p.sync(0.0, true, false);
            while start.elapsed() < Duration::from_secs(secs) {
                std::thread::sleep(Duration::from_millis(250));
                p.sync(start.elapsed().as_secs_f64(), true, false);
            }
            // End of the song (pause), as the UI does before finishing; the
            // stats call below must not count as played time.
            p.sync(start.elapsed().as_secs_f64(), false, false);
            let during = proc_stats();
            // finish() step by step (same code path) to attribute its cost.
            let t = Instant::now();
            let now = p.capture.captured_samples();
            let LivePerformance { capture, session } = p;
            drop(capture);
            let teardown_ms = t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let sum = lock(&session).summary(now);
            let summary_ms = t.elapsed().as_secs_f64() * 1000.0;
            let after = proc_stats();
            eprintln!(
                "PERF run {run}: threads during {} after {}; private MB during {:.1} after {:.1}; finish = device teardown {teardown_ms:.1} ms + summary {summary_ms:.2} ms; played {} ms",
                during.threads, after.threads, during.private_mb, after.private_mb, sum.played_ms
            );
            rows.push((during, after));
        }
        let first = rows[0].1;
        let last = rows[9].1;
        eprintln!(
            "PERF stability: baseline threads {} private {:.1} MB; after run 1 {} / {:.1}; after run 10 {} / {:.1}",
            baseline.threads,
            baseline.private_mb,
            first.threads,
            first.private_mb,
            last.threads,
            last.private_mb
        );
        // Our analysis thread and cpal's stream thread are joined on drop. What
        // remains varies with the Windows thread pool used by WASAPI (measured:
        // plateaus of 7 to 10). A per-run leak would add one thread per run.
        assert!(
            rows[1..].iter().all(|r| r.1.threads <= first.threads + 5),
            "threads leaked"
        );
        assert!(last.private_mb - first.private_mb < 5.0, "memory grew");
    }
}
