//! Deterministic vocal scoring, algorithm v1 (SPEC-006). Every constant below is
//! quoted from the spec; changing any result requires bumping `ALGORITHM_VERSION`.

use crate::MelodyReference;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const ALGORITHM_VERSION: u32 = 1;

/// Median window (frames) applied to f0 before computing the cents error.
const MEDIAN_FRAMES: usize = 5;
const MEDIAN_HALF: usize = MEDIAN_FRAMES / 2;
/// Onset search window: [start − 150 ms, start + dur/2].
const ONSET_EARLY_MS: f64 = 150.0;
/// Rhythm ramp: full credit ≤ 80 ms, zero ≥ 300 ms.
const RHYTHM_FULL_MS: f64 = 80.0;
const RHYTHM_ZERO_MS: f64 = 300.0;
const LIVE_WINDOW_MS: f64 = 200.0;
const W_PITCH: f64 = 0.65;
const W_RHYTHM: f64 = 0.20;
const W_SUSTAIN: f64 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    /// (T_full, T_zero) in cents.
    fn tolerances(self) -> (f64, f64) {
        match self {
            Difficulty::Easy => (75.0, 200.0),
            Difficulty::Normal => (50.0, 150.0),
            Difficulty::Hard => (30.0, 100.0),
        }
    }
}

/// One analysis frame mapped to song time (offsets and latency already applied).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceFrame {
    pub t_ms: f64,
    pub f0_hz: Option<f32>,
    pub confidence: f32,
    pub voiced: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedbackLabel {
    Excellent,
    VeryGood,
    Good,
    NeedsWork,
    NotDetected,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveFeedback {
    /// Time of the frame this feedback refers to (2 frames behind the newest, median lookahead).
    pub t_ms: f64,
    pub target_midi: f32,
    /// Smoothed sung pitch, not octave-folded.
    pub sung_midi: Option<f32>,
    /// Octave-folded error in (−600, 600] cents.
    pub cents_error: Option<f32>,
    pub label: FeedbackLabel,
    /// Score (0..100) over notes already finished.
    pub running_score: f32,
}

/// `pitch`, `rhythm`, `sustain` and `coverage` are 0..1; `score` is 0..100 rounded to 0.1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreResult {
    pub score: f32,
    pub pitch: f32,
    pub rhythm: f32,
    pub sustain: f32,
    pub coverage: f32,
    pub notes_total: u32,
    pub notes_hit: u32,
    pub algorithm_version: u32,
}

#[derive(Debug, Clone)]
struct NoteState {
    start: f64,
    end: f64,
    midi: f64,
    frames: u32,
    p_sum: f64,
    sustained: u32,
    onset: Option<f64>,
}

impl NoteState {
    fn dur(&self) -> f64 {
        self.end - self.start
    }
    fn pitch(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            self.p_sum / self.frames as f64
        }
    }
    fn sustain(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            self.sustained as f64 / self.frames as f64
        }
    }
    fn rhythm(&self) -> f64 {
        self.onset.map_or(0.0, |o| {
            ramp((o - self.start).abs(), RHYTHM_FULL_MS, RHYTHM_ZERO_MS)
        })
    }
    fn weighted(&self) -> f64 {
        W_PITCH * self.pitch() + W_RHYTHM * self.rhythm() + W_SUSTAIN * self.sustain()
    }
}

/// Incremental scorer. Feed frames in time order with [`Scorer::push`]; call
/// [`Scorer::finish`] at the end. Same frames ⇒ same result.
pub struct Scorer {
    notes: Vec<NoteState>,
    tol: (f64, f64),
    raw: Vec<VoiceFrame>,
    next: usize,
    recent: VecDeque<(f64, f64, bool)>,
    finalized: usize,
    done_dur: f64,
    done_weighted: f64,
}

impl Scorer {
    pub fn new(reference: &MelodyReference, offset_ms: i32, difficulty: Difficulty) -> Self {
        let notes = reference
            .notes
            .iter()
            .map(|n| {
                let start = n.start_ms as f64 + offset_ms as f64;
                NoteState {
                    start,
                    end: start + n.dur_ms as f64,
                    midi: n.midi as f64,
                    frames: 0,
                    p_sum: 0.0,
                    sustained: 0,
                    onset: None,
                }
            })
            .collect();
        Self {
            notes,
            tol: difficulty.tolerances(),
            raw: Vec::new(),
            next: 0,
            recent: VecDeque::new(),
            finalized: 0,
            done_dur: 0.0,
            done_weighted: 0.0,
        }
    }

    /// Returns feedback for the frame two positions back (centered median needs
    /// two frames of lookahead) when it falls inside a note.
    pub fn push(&mut self, frame: VoiceFrame) -> Option<LiveFeedback> {
        self.raw.push(frame);
        if self.raw.len() > self.next + MEDIAN_HALF {
            let fb = self.process(self.next);
            self.next += 1;
            return fb;
        }
        None
    }

    pub fn finish(mut self) -> ScoreResult {
        while self.next < self.raw.len() {
            self.process(self.next);
            self.next += 1;
        }
        let total: f64 = self.notes.iter().map(NoteState::dur).sum();
        let avg = |f: fn(&NoteState) -> f64| {
            if total > 0.0 {
                self.notes.iter().map(|n| n.dur() * f(n)).sum::<f64>() / total
            } else {
                0.0
            }
        };
        let (p, r, s) = (
            avg(NoteState::pitch),
            avg(NoteState::rhythm),
            avg(NoteState::sustain),
        );
        let hit = self.notes.iter().filter(|n| n.pitch() >= 0.5).count() as u32;
        let notes_total = self.notes.len() as u32;
        ScoreResult {
            score: round1(100.0 * (W_PITCH * p + W_RHYTHM * r + W_SUSTAIN * s)),
            pitch: p as f32,
            rhythm: r as f32,
            sustain: s as f32,
            coverage: if notes_total == 0 {
                0.0
            } else {
                hit as f32 / notes_total as f32
            },
            notes_total,
            notes_hit: hit,
            algorithm_version: ALGORITHM_VERSION,
        }
    }

    fn process(&mut self, i: usize) -> Option<LiveFeedback> {
        let frame = self.raw[i];
        let t = frame.t_ms;
        let sung = self.smoothed_midi(i);
        let tol = self.tol;
        let p_for = |midi: f64| sung.map_or(0.0, |s| pitch_credit(cents(s, midi), tol));

        // First note starting after t; the note containing t (if any) is the one before.
        let k = self.notes.partition_point(|n| n.start <= t);
        for n in &mut self.notes[k.saturating_sub(1)..] {
            if n.start - ONSET_EARLY_MS > t {
                break;
            }
            if n.onset.is_none() && t <= n.start + n.dur() / 2.0 && p_for(n.midi) >= 0.5 {
                n.onset = Some(t);
            }
        }

        let active = k.checked_sub(1).filter(|&j| t < self.notes[j].end);
        let feedback = active.map(|j| {
            let n = &mut self.notes[j];
            let p = p_for(n.midi);
            n.frames += 1;
            n.p_sum += p;
            n.sustained += u32::from(p >= 0.5);
            let target = n.midi;

            self.recent
                .retain(|&(rt, _, _)| rt > t - LIVE_WINDOW_MS && rt <= t);
            self.recent.push_back((t, p, sung.is_some()));
            (target, p)
        });

        while self.finalized < self.notes.len() && self.notes[self.finalized].end <= t {
            let n = &self.notes[self.finalized];
            self.done_dur += n.dur();
            self.done_weighted += n.dur() * n.weighted();
            self.finalized += 1;
        }

        feedback.map(|(target, _)| LiveFeedback {
            t_ms: t,
            target_midi: target as f32,
            sung_midi: sung.map(|s| s as f32),
            cents_error: sung.map(|s| cents(s, target) as f32),
            label: self.label(),
            running_score: if self.done_dur > 0.0 {
                round1(100.0 * self.done_weighted / self.done_dur)
            } else {
                0.0
            },
        })
    }

    /// Median (in MIDI units) of the voiced f0 values in the centered 5-frame
    /// window; `None` if this frame itself is not voiced.
    fn smoothed_midi(&self, i: usize) -> Option<f64> {
        let own = self.raw[i];
        own.f0_hz.filter(|_| own.voiced)?;
        let lo = i.saturating_sub(MEDIAN_HALF);
        let hi = (i + MEDIAN_HALF).min(self.raw.len() - 1);
        let mut v: Vec<f64> = self.raw[lo..=hi]
            .iter()
            .filter(|f| f.voiced)
            .filter_map(|f| f.f0_hz)
            .filter(|&hz| hz > 0.0)
            .map(|hz| hz_to_midi(hz as f64))
            .collect();
        if v.is_empty() {
            return None;
        }
        v.sort_by(f64::total_cmp);
        let m = v.len() / 2;
        Some(if v.len() % 2 == 1 {
            v[m]
        } else {
            (v[m - 1] + v[m]) / 2.0
        })
    }

    fn label(&self) -> FeedbackLabel {
        let n = self.recent.len();
        let voiced = self.recent.iter().filter(|r| r.2).count();
        if n == 0 || voiced * 2 < n {
            return FeedbackLabel::NotDetected;
        }
        let avg = self.recent.iter().map(|r| r.1).sum::<f64>() / n as f64;
        match avg {
            a if a >= 0.9 => FeedbackLabel::Excellent,
            a if a >= 0.75 => FeedbackLabel::VeryGood,
            a if a >= 0.5 => FeedbackLabel::Good,
            _ => FeedbackLabel::NeedsWork,
        }
    }
}

fn hz_to_midi(hz: f64) -> f64 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

/// Octave-folded error in cents, range (−600, 600].
fn cents(sung_midi: f64, target_midi: f64) -> f64 {
    let e = 100.0 * (sung_midi - target_midi);
    let folded = e - 1200.0 * (e / 1200.0).round();
    if folded <= -600.0 {
        folded + 1200.0
    } else {
        folded
    }
}

fn pitch_credit(cents: f64, (full, zero): (f64, f64)) -> f64 {
    ramp(cents.abs(), full, zero)
}

/// 1 at or below `full`, 0 at or above `zero`, linear between.
fn ramp(x: f64, full: f64, zero: f64) -> f64 {
    ((zero - x) / (zero - full)).clamp(0.0, 1.0)
}

fn round1(x: f64) -> f32 {
    ((x * 10.0).round() / 10.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Note;

    fn reference(notes: &[(u32, u32, f32)]) -> MelodyReference {
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
        }
    }

    fn midi_hz(m: f64) -> f32 {
        (440.0 * 2f64.powf((m - 69.0) / 12.0)) as f32
    }

    /// Frames every 10 ms over [0, end_ms), pitch from `f(t)` (None = unvoiced).
    fn run(
        r: &MelodyReference,
        d: Difficulty,
        end_ms: u32,
        f: impl Fn(f64) -> Option<f64>,
    ) -> ScoreResult {
        let mut s = Scorer::new(r, 0, d);
        for i in 0..end_ms / 10 {
            let t = i as f64 * 10.0;
            let m = f(t);
            s.push(VoiceFrame {
                t_ms: t,
                f0_hz: m.map(midi_hz),
                confidence: 0.95,
                voiced: m.is_some(),
            });
        }
        s.finish()
    }

    #[test]
    fn cents_fold_octaves_into_half_open_range() {
        assert!((cents(69.0, 69.0)).abs() < 1e-9);
        assert!((cents(57.0, 69.0)).abs() < 1e-9); // octave below
        assert!((cents(82.0, 69.0) - 100.0).abs() < 1e-9); // octave + semitone
        assert!((cents(75.0, 69.0) - 600.0).abs() < 1e-9);
        assert!((cents(63.0, 69.0) - 600.0).abs() < 1e-9); // −600 maps to +600
    }

    #[test]
    fn tolerance_ramps_per_difficulty() {
        let n = Difficulty::Normal.tolerances();
        assert_eq!(pitch_credit(50.0, n), 1.0);
        assert_eq!(pitch_credit(-100.0, n), 0.5);
        assert_eq!(pitch_credit(150.0, n), 0.0);
        assert_eq!(pitch_credit(100.0, Difficulty::Easy.tolerances()), 0.8);
        assert_eq!(pitch_credit(65.0, Difficulty::Hard.tolerances()), 0.5);
    }

    #[test]
    fn exact_frames_give_exact_metrics() {
        let r = reference(&[(1000, 500, 60.0), (2000, 500, 64.0)]);
        let perfect = run(&r, Difficulty::Normal, 3000, |t| {
            if (1000.0..1500.0).contains(&t) {
                Some(60.0)
            } else if (2000.0..2500.0).contains(&t) {
                Some(64.0)
            } else {
                None
            }
        });
        assert_eq!(perfect.score, 100.0);
        assert_eq!((perfect.notes_hit, perfect.notes_total), (2, 2));

        // +98 cents everywhere on Normal: p = 1 − (98 − 50)/100 = 0.52 on every frame.
        // (Exactly +100 sits on the p ≥ 0.5 boundary, where f32 Hz rounding decides.)
        let sharp = run(&r, Difficulty::Normal, 3000, |t| {
            if (1000.0..1500.0).contains(&t) {
                Some(60.98)
            } else if (2000.0..2500.0).contains(&t) {
                Some(64.98)
            } else {
                None
            }
        });
        assert!((sharp.pitch - 0.52).abs() < 1e-4);
        assert_eq!(sharp.sustain, 1.0); // p ≥ 0.5 counts as sustained
        assert_eq!(sharp.rhythm, 1.0);
        assert_eq!(sharp.score, 68.8); // 100·(0.65·0.52 + 0.2 + 0.15)
    }

    #[test]
    fn unsung_notes_count_as_zero_weighted_by_duration() {
        // 1 of 2 notes sung perfectly; the sung one is 1/4 of the total duration.
        let r = reference(&[(0, 500, 60.0), (1000, 1500, 62.0)]);
        let res = run(&r, Difficulty::Normal, 3000, |t| {
            (t < 500.0).then_some(60.0)
        });
        assert!((res.pitch - 0.25).abs() < 1e-6);
        assert_eq!(res.score, 25.0);
        assert_eq!(res.coverage, 0.5);
    }

    #[test]
    fn onset_ramp_and_early_window() {
        let r = reference(&[(1000, 1000, 60.0)]);
        let at = |delay: f64| {
            run(&r, Difficulty::Normal, 2500, move |t| {
                (t >= 1000.0 + delay && t < 2000.0).then_some(60.0)
            })
            .rhythm
        };
        assert_eq!(at(0.0), 1.0);
        assert_eq!(at(80.0), 1.0);
        assert!((at(190.0) - 0.5).abs() < 1e-6);
        assert_eq!(at(300.0), 0.0);
        // Early entry inside the 150 ms window counts as the onset.
        let early = run(&r, Difficulty::Normal, 2500, |t| {
            (900.0..2000.0).contains(&t).then_some(60.0)
        });
        assert!((early.rhythm - (1.0 - 20.0 / 220.0)).abs() < 1e-6);
    }

    #[test]
    fn median_absorbs_two_frame_glitches() {
        let r = reference(&[(0, 1000, 60.0)]);
        let res = run(&r, Difficulty::Hard, 1000, |t| {
            Some(if (500.0..520.0).contains(&t) {
                72.0 + 7.0
            } else {
                60.0
            })
        });
        assert_eq!(res.pitch, 1.0);
    }

    #[test]
    fn live_feedback_labels_and_running_score() {
        let r = reference(&[(0, 500, 60.0), (600, 500, 62.0)]);
        let mut s = Scorer::new(&r, 0, Difficulty::Normal);
        let mut last = Vec::new();
        for i in 0..110 {
            let t = i as f64 * 10.0;
            let m = if t < 500.0 { Some(60.0) } else { None };
            if let Some(fb) = s.push(VoiceFrame {
                t_ms: t,
                f0_hz: m.map(midi_hz),
                confidence: 0.9,
                voiced: m.is_some(),
            }) {
                last.push(fb);
            }
        }
        let first = last.iter().find(|f| f.t_ms == 100.0).unwrap();
        assert_eq!(first.label, FeedbackLabel::Excellent);
        assert!(first.cents_error.is_some_and(|c| c.abs() < 0.01));
        assert_eq!(first.running_score, 0.0);
        let second = last.iter().find(|f| f.t_ms == 800.0).unwrap();
        assert_eq!(second.label, FeedbackLabel::NotDetected);
        assert_eq!(second.target_midi, 62.0);
        assert_eq!(second.running_score, 100.0); // only the finished, perfect first note
        assert!(last.iter().all(|f| f.t_ms < 500.0 || f.t_ms >= 600.0)); // none in the gap
    }

    #[test]
    fn offset_shifts_the_reference() {
        let r = reference(&[(1000, 500, 60.0)]);
        let mut s = Scorer::new(&r, -500, Difficulty::Normal);
        for i in 0..150 {
            let t = i as f64 * 10.0;
            let m = (500.0..1000.0).contains(&t).then_some(60.0);
            s.push(VoiceFrame {
                t_ms: t,
                f0_hz: m.map(midi_hz),
                confidence: 0.9,
                voiced: m.is_some(),
            });
        }
        assert_eq!(s.finish().score, 100.0);
    }

    #[test]
    fn empty_reference_scores_zero_without_nan() {
        let res = run(&reference(&[]), Difficulty::Normal, 500, |_| Some(60.0));
        assert_eq!(
            (res.score, res.pitch, res.coverage, res.notes_total),
            (0.0, 0.0, 0.0, 0)
        );
    }
}
