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
use audio_engine::{AnalysisFrame, Capture, CaptureConfig, window_len};
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
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSummary {
    /// Validated reference, a result, and no seek. The command still requires
    /// `played_ms ≥ 50%` of the song (SPEC-007 R2).
    pub competitive: bool,
    /// Song time actually played (playing = true), skipped stretches excluded.
    pub played_ms: u64,
    pub result: Option<ScoreResult>,
    /// Set when there is no reference.
    pub exploratory: Option<ExploratoryMetrics>,
    pub capture: CaptureStats,
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
        let session = Arc::new(Mutex::new(Session::new(&cfg)));
        let shared = session.clone();
        let capture = Capture::start(cfg.device_id.as_deref(), cfg.capture, move |f| {
            let tick = lock(&shared).on_frame(&f);
            if let Some(t) = tick {
                on_tick(t);
            }
        })
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

#[derive(Debug, Clone, Copy)]
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
    frames_seen: u64,
    scored_frames: u32,
    clipped_frames: u32,
    outside_frames: u32,
    outside_voiced: u32,
    feedback: Option<LiveFeedback>,
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
        Self {
            competitive: cfg.competitive && scorer.is_some(),
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
        }
        self.last_video_ms = video_ms;
        if seeked {
            // Seek = training (overview §6). Notes jumped over are ignored, not zeroed.
            // ponytail: a backward seek re-sings notes on top of their first pass;
            // fine for training, revisit if seeks ever count.
            self.competitive = false;
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
            }
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
        if let Some(a) = self.anchor {
            // Tail since the last sync, estimated on the mic clock (≤ one sync period).
            self.played_ms += (self.video_ms_at(a, now) - self.last_video_ms).max(0.0);
        }
        self.close_segment();
        let result = self.scorer.take().map(Scorer::finish);
        let ratio = |n: u32, d: u32| if d == 0 { 0.0 } else { n as f32 / d as f32 };
        PerformanceSummary {
            competitive: self.competitive && result.is_some(),
            played_ms: self.played_ms.round() as u64,
            exploratory: result.is_none().then(|| exploratory_metrics(&self.voice)),
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
            },
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
        let notes = [(0, 500, 60.0), (1000, 500, 62.0), (5000, 500, 64.0)];
        let mut s = session(Some(&notes), true, 0.0);
        s.sync(0.0, true, false, 0);
        feed(&mut s, 0, 28_800, Some(midi_hz(60.0))); // sing 0–0.6 s
        s.sync(5.0, true, true, 28_800); // seek 0.6 s → 5 s
        feed(&mut s, 28_800, 57_600, Some(midi_hz(64.0))); // sing 5–5.6 s
        let sum = s.summary(57_600);
        let r = sum.result.unwrap();
        assert!(!sum.competitive);
        assert_eq!((r.notes_total, r.notes_hit), (2, 2));
        assert!(r.score >= 99.0, "{r:?}");
        assert_eq!(sum.played_ms, 1200);
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
}
