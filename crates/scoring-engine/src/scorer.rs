//! Deterministic vocal scoring, algorithm v2 (SPEC-006 "Algoritmo v2"). Every
//! constant below is quoted from the spec; changing any result requires bumping
//! `ALGORITHM_VERSION` and the golden values in `tests/regression.rs`.
//!
//! Per note i (duration d, scored region = note minus transition zones):
//! - `p(frame)` = 1 within T_full cents, 0 beyond T_zero, linear between
//!   (octave-folded error of the in-note median pitch; unvoiced = 0).
//! - `pitch_i` = Σp / expected frames (missing frames count 0).
//! - `accuracy_i` = mean p over the voiced frames (how in tune, when singing).
//! - `gate_i` = clamp(2·accuracy_i − 1) · min(1, 2·pitch_i): continuous; 0 for
//!   notes sung mostly off-pitch (sirens) or barely sung at all.
//! - `rhythm_i` = timing(onset) · gate_i; `sustain_i` = held_i · gate_i.
//!
//! Score = 100·(0.65·P + 0.20·R + 0.15·S), duration-weighted over all notes.

use crate::MelodyReference;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const ALGORITHM_VERSION: u32 = 2;

/// Frame period of the analysis (contract: one frame every 10 ms).
const HOP_MS: f64 = 10.0;
/// Median window (frames) applied to f0 before computing the cents error.
const MEDIAN_FRAMES: usize = 5;
const MEDIAN_HALF: usize = MEDIAN_FRAMES / 2;
/// Onset search starts at max(start − 300 ms, end of the previous note + guard).
const ONSET_EARLY_MS: f64 = 300.0;
/// Right after a note ends the analysis window (≈ 43 ms, dated at its centre)
/// still contains it: half a window is release smear, never a new attack.
const ONSET_GUARD_MS: f64 = 25.0;
/// Rhythm ramp (symmetric): full credit ≤ 80 ms, zero ≥ 300 ms.
const RHYTHM_FULL_MS: f64 = 80.0;
const RHYTHM_ZERO_MS: f64 = 300.0;
/// Notes closer than this are "tied": the boundary gets a transition zone.
const TIED_GAP_MS: f64 = 30.0;
/// Transition zone length: min(25 ms, d/4).
const TRANSITION_MS: f64 = 25.0;
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
    /// (T_full, T_zero) in cents. Widths ≥ 135 cents keep the score slope for a
    /// constant detune ≤ 10 points per 10 cents (no cliffs); Normal puts +90 cents
    /// at ≤ 40 and +100 cents at P ≈ 0.46.
    fn tolerances(self) -> (f64, f64) {
        match self {
            Difficulty::Easy => (40.0, 200.0),
            Difficulty::Normal => (25.0, 165.0),
            Difficulty::Hard => (15.0, 155.0),
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
    /// Scored region: the note minus transition zones at tied boundaries.
    region: (f64, f64),
    expected: u32,
    onset_from: f64,
    /// Target of the previous note: an onset must be nearer this note than that one.
    prev_midi: Option<f64>,
    received: u32,
    voiced: u32,
    held: u32,
    p_sum: f64,
    onset: Option<f64>,
    /// Jumped over by a seek (training only): excluded from every aggregate.
    skipped: bool,
}

impl NoteState {
    fn dur(&self) -> f64 {
        self.end - self.start
    }
    /// Frames that should have been scored; never below what was received.
    fn denom(&self) -> f64 {
        self.expected.max(self.received).max(1) as f64
    }
    fn pitch(&self) -> f64 {
        self.p_sum / self.denom()
    }
    fn accuracy(&self) -> f64 {
        if self.voiced == 0 {
            0.0
        } else {
            self.p_sum / self.voiced as f64
        }
    }
    fn gate(&self) -> f64 {
        (2.0 * self.accuracy() - 1.0).clamp(0.0, 1.0) * (2.0 * self.pitch()).min(1.0)
    }
    fn sustain(&self) -> f64 {
        self.held as f64 / self.denom() * self.gate()
    }
    fn rhythm(&self) -> f64 {
        self.onset.map_or(0.0, |o| {
            ramp((o - self.start).abs(), RHYTHM_FULL_MS, RHYTHM_ZERO_MS)
        }) * self.gate()
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
        let spans: Vec<(f64, f64, f64)> = reference
            .notes
            .iter()
            .map(|n| {
                let start = n.start_ms as f64 + offset_ms as f64;
                (start, start + n.dur_ms as f64, n.midi as f64)
            })
            .collect();
        let notes = spans
            .iter()
            .enumerate()
            .map(|(i, &(start, end, midi))| {
                let prev_end = i.checked_sub(1).map(|j| spans[j].1);
                let next_start = spans.get(i + 1).map(|n| n.0);
                let zone = TRANSITION_MS.min((end - start) / 4.0);
                let tied_before = prev_end.is_some_and(|p| start - p < TIED_GAP_MS);
                let tied_after = next_start.is_some_and(|n| n - end < TIED_GAP_MS);
                let region = (
                    start + if tied_before { zone } else { 0.0 },
                    end - if tied_after { zone } else { 0.0 },
                );
                NoteState {
                    start,
                    end,
                    midi,
                    region,
                    // floor: a 10 ms grid always delivers at least this many frames in the region
                    expected: ((region.1 - region.0) / HOP_MS).floor() as u32,
                    onset_from: prev_end.map_or(start - ONSET_EARLY_MS, |p| {
                        (p + ONSET_GUARD_MS).min(start).max(start - ONSET_EARLY_MS)
                    }),
                    prev_midi: i.checked_sub(1).map(|j| spans[j].2),
                    received: 0,
                    voiced: 0,
                    held: 0,
                    p_sum: 0.0,
                    onset: None,
                    skipped: false,
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

    /// Training only (a seek jumped from `from_ms` to `to_ms`, song time): notes
    /// starting in `[from_ms, to_ms)` are excluded from the result instead of
    /// counting as unsung. Runs without seeks are unaffected.
    pub fn skip(&mut self, from_ms: f64, to_ms: f64) {
        for n in &mut self.notes {
            if n.start >= from_ms && n.start < to_ms {
                n.skipped = true;
            }
        }
    }

    pub fn finish(mut self) -> ScoreResult {
        while self.next < self.raw.len() {
            self.process(self.next);
            self.next += 1;
        }
        let notes: Vec<&NoteState> = self.notes.iter().filter(|n| !n.skipped).collect();
        let total: f64 = notes.iter().map(|n| n.dur()).sum();
        let avg = |f: fn(&NoteState) -> f64| {
            if total > 0.0 {
                notes.iter().map(|n| n.dur() * f(n)).sum::<f64>() / total
            } else {
                0.0
            }
        };
        let (p, r, s) = (
            avg(NoteState::pitch),
            avg(NoteState::rhythm),
            avg(NoteState::sustain),
        );
        let hit = notes.iter().filter(|n| n.pitch() >= 0.5).count() as u32;
        let notes_total = notes.len() as u32;
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
        let t = self.raw[i].t_ms;
        let tol = self.tol;
        let credit =
            |sung: Option<f64>, midi: f64| sung.map_or(0.0, |s| pitch_credit(cents(s, midi), tol));
        // Free median: onset detection and the live label outside notes.
        let sung = self.smoothed_midi(i, f64::NEG_INFINITY, f64::INFINITY);

        // First note starting after t; the note containing t (if any) is the one before.
        let k = self.notes.partition_point(|n| n.start <= t);
        for n in &mut self.notes[k.saturating_sub(1)..] {
            if n.onset_from > t {
                break;
            }
            // Still nearer the previous target = has not moved to this note yet.
            let moved_on = sung.is_some_and(|s| {
                n.prev_midi
                    .is_none_or(|pm| cents(s, n.midi).abs() <= cents(s, pm).abs())
            });
            if n.onset.is_none()
                && t <= n.start + n.dur() / 2.0
                && credit(sung, n.midi) >= 0.5
                && moved_on
            {
                n.onset = Some(t);
            }
        }

        let active = k.checked_sub(1).filter(|&j| t < self.notes[j].end);
        let feedback = active.map(|j| {
            // In-note median: neighbours' pitches never leak across a tied boundary.
            let (start, end) = (self.notes[j].start, self.notes[j].end);
            let sung_in = self.smoothed_midi(i, start, end);
            let n = &mut self.notes[j];
            let p = credit(sung_in, n.midi);
            if t >= n.region.0 && t < n.region.1 {
                n.received += 1;
                n.p_sum += p;
                n.voiced += u32::from(sung_in.is_some());
                n.held += u32::from(p > 0.0);
            }
            let target = n.midi;
            self.recent
                .retain(|&(rt, _, _)| rt > t - LIVE_WINDOW_MS && rt <= t);
            self.recent.push_back((t, p, sung_in.is_some()));
            (target, sung_in)
        });

        while self.finalized < self.notes.len() && self.notes[self.finalized].end <= t {
            let n = &self.notes[self.finalized];
            if !n.skipped {
                self.done_dur += n.dur();
                self.done_weighted += n.dur() * n.weighted();
            }
            self.finalized += 1;
        }

        feedback.map(|(target, sung_in)| LiveFeedback {
            t_ms: t,
            target_midi: target as f32,
            sung_midi: sung_in.map(|s| s as f32),
            cents_error: sung_in.map(|s| cents(s, target) as f32),
            label: self.label(),
            running_score: if self.done_dur > 0.0 {
                round1(100.0 * self.done_weighted / self.done_dur)
            } else {
                0.0
            },
        })
    }

    /// Median (in MIDI units) of the voiced f0 values in the centered 5-frame
    /// window, using only frames with time in `[from, to)`; `None` if this frame
    /// itself is not voiced.
    fn smoothed_midi(&self, i: usize, from: f64, to: f64) -> Option<f64> {
        let own = self.raw[i];
        own.f0_hz.filter(|_| own.voiced)?;
        let lo = i.saturating_sub(MEDIAN_HALF);
        let hi = (i + MEDIAN_HALF).min(self.raw.len() - 1);
        let mut v: Vec<f64> = self.raw[lo..=hi]
            .iter()
            .filter(|f| f.voiced && f.t_ms >= from && f.t_ms < to)
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

    fn within(t: f64, a: f64, b: f64) -> bool {
        (a..b).contains(&t)
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
        assert_eq!(pitch_credit(25.0, n), 1.0);
        assert_eq!(pitch_credit(-95.0, n), 0.5);
        assert_eq!(pitch_credit(165.0, n), 0.0);
        assert_eq!(pitch_credit(120.0, Difficulty::Easy.tolerances()), 0.5);
        assert_eq!(pitch_credit(85.0, Difficulty::Hard.tolerances()), 0.5);
    }

    #[test]
    fn exact_frames_give_exact_metrics() {
        let r = reference(&[(1000, 500, 60.0), (2000, 500, 64.0)]);
        let sing = |off: f64| {
            move |t: f64| {
                if within(t, 1000.0, 1500.0) {
                    Some(60.0 + off)
                } else if within(t, 2000.0, 2500.0) {
                    Some(64.0 + off)
                } else {
                    None
                }
            }
        };
        let perfect = run(&r, Difficulty::Normal, 3000, sing(0.0));
        assert_eq!(perfect.score, 100.0);
        assert_eq!((perfect.notes_hit, perfect.notes_total), (2, 2));

        // +60 cents: p = 1 − 35/140 = 0.75, gate = 0.5 → 100·(0.65·0.75 + 0.2·0.5 + 0.15·0.5).
        let sharp = run(&r, Difficulty::Normal, 3000, sing(0.6));
        assert!((sharp.pitch - 0.75).abs() < 1e-4, "{sharp:?}");
        assert!((sharp.rhythm - 0.5).abs() < 1e-3, "{sharp:?}");
        assert!((sharp.sustain - 0.5).abs() < 1e-3, "{sharp:?}");
        assert_eq!(sharp.score, 66.2); // 66.25 in f64 rounds down
    }

    /// SPEC-006 v2 A4 constraints on a constant detune (ideal frames, Normal).
    #[test]
    fn constant_detune_curve_is_monotone_without_cliffs() {
        let r = reference(&[(500, 1000, 60.0), (1700, 1000, 64.0), (2900, 1000, 67.0)]);
        let score_at = |c: f64| {
            run(&r, Difficulty::Normal, 4500, |t| {
                r.notes
                    .iter()
                    .find(|n| within(t, n.start_ms as f64, (n.start_ms + n.dur_ms) as f64))
                    .map(|n| n.midi as f64 + c / 100.0)
            })
            .score
        };
        let curve: Vec<f32> = (0..=300).map(|c| score_at(c as f64)).collect();
        for c in 1..curve.len() {
            assert!(curve[c] <= curve[c - 1], "not monotone at {c} cents");
        }
        for c in 10..curve.len() {
            let jump = curve[c - 10] - curve[c];
            assert!(
                jump <= 10.0,
                "{jump} points between {} and {c} cents",
                c - 10
            );
        }
        assert!(curve[90] <= 40.0, "+90 cents = {}", curve[90]);
        let max_jump = (10..curve.len())
            .map(|c| curve[c - 10] - curve[c])
            .fold(0f32, f32::max);
        eprintln!(
            "A4 curve: max jump over 10 cents = {max_jump:.1}; +90 = {}",
            curve[90]
        );
        assert_eq!(curve[0], 100.0);
        assert_eq!(curve[25], 100.0);
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
    fn onset_is_symmetric_within_300ms_and_bounded_by_the_previous_note() {
        let r = reference(&[(1000, 1000, 60.0)]);
        let shifted = |delay: f64| {
            run(&r, Difficulty::Normal, 2600, move |t| {
                within(t, 1000.0 + delay, 2000.0 + delay.min(0.0)).then_some(60.0)
            })
            .rhythm
        };
        assert_eq!(shifted(0.0), 1.0);
        assert_eq!(shifted(80.0), 1.0);
        assert!((shifted(190.0) - 0.5).abs() < 1e-6);
        assert!((shifted(-190.0) - 0.5).abs() < 1e-6); // early costs as much as late
        assert_eq!(shifted(-300.0), 0.0);
        // Previous note ends 200 ms before the next: the search starts 25 ms after
        // its end (release guard). The singer enters note 2 at 750 ms, but its
        // window opens at 825; first 10 ms frame there is 830 → err 170 → 1 − 90/220.
        let r2 = reference(&[(0, 800, 55.0), (1000, 1000, 60.0)]);
        let early = run(&r2, Difficulty::Normal, 2500, |t| {
            if t < 700.0 {
                Some(55.0)
            } else {
                within(t, 750.0, 2000.0).then_some(60.0)
            }
        });
        // Note 1 also hears the early entry (750–800 ms, 500 cents off): accuracy
        // 70/75 → gate 2·70/75 − 1.
        let gate1 = 2.0 * 70.0 / 75.0 - 1.0;
        let expected = (800.0 * gate1 + 1000.0 * (1.0 - 90.0 / 220.0)) / 1800.0;
        assert!((early.rhythm as f64 - expected).abs() < 1e-3, "{early:?}");
    }

    #[test]
    fn repeated_tied_notes_keep_full_rhythm() {
        // A1: four tied 500 ms notes on the same pitch, held perfectly.
        let r = reference(&[
            (500, 500, 60.0),
            (1000, 500, 60.0),
            (1500, 500, 60.0),
            (2000, 500, 60.0),
        ]);
        let res = run(&r, Difficulty::Normal, 3000, |t| {
            within(t, 500.0, 2500.0).then_some(60.0)
        });
        assert_eq!(res.rhythm, 1.0);
        assert_eq!(res.score, 100.0);
    }

    #[test]
    fn siren_and_fixed_note_get_no_rhythm_or_sustain_on_missed_notes() {
        let r = reference(&[(500, 1000, 60.0), (1700, 1000, 64.0), (2900, 1000, 67.0)]);
        let siren = run(&r, Difficulty::Normal, 4500, |t| {
            let x = (t % 400.0) / 400.0;
            Some(60.0 + 12.0 * (1.0 - (2.0 * x - 1.0).abs()))
        });
        assert!(siren.rhythm < 0.05 && siren.sustain < 0.05, "{siren:?}");
        assert!(siren.score <= 15.0, "{siren:?}");
    }

    #[test]
    fn transition_zones_ignore_tied_boundaries() {
        // Tied 100 ms notes; the singer arrives 20 ms late on each (inside the zone).
        let notes: Vec<(u32, u32, f32)> = (0..10)
            .map(|i| (500 + i * 100, 100, if i % 2 == 0 { 60.0 } else { 64.0 }))
            .collect();
        let r = reference(&notes);
        let res = run(&r, Difficulty::Normal, 2000, |t| {
            let i = ((t - 500.0) / 100.0).floor();
            if !(0.0..10.0).contains(&i) {
                return None;
            }
            let late = t - (500.0 + i * 100.0) < 20.0 && i > 0.0;
            let i = if late { i - 1.0 } else { i };
            Some(if (i as u32).is_multiple_of(2) {
                60.0
            } else {
                64.0
            })
        });
        assert!(res.pitch > 0.99, "{res:?}");
    }

    #[test]
    fn missing_frames_count_as_zero() {
        // A8: only the first 100 ms of a 1000 ms note arrive, sung perfectly.
        let r = reference(&[(500, 1000, 60.0)]);
        let mut s = Scorer::new(&r, 0, Difficulty::Normal);
        for i in 0..200 {
            let t = i as f64 * 10.0;
            if t < 600.0 || t >= 1600.0 {
                let m = within(t, 500.0, 600.0).then_some(60.0);
                s.push(VoiceFrame {
                    t_ms: t,
                    f0_hz: m.map(midi_hz),
                    confidence: 0.9,
                    voiced: m.is_some(),
                });
            }
        }
        let res = s.finish();
        assert!((res.pitch - 0.1).abs() < 0.01, "{res:?}");
        assert!(res.score < 30.0, "{res:?}");
    }

    #[test]
    fn median_absorbs_two_frame_glitches() {
        let r = reference(&[(0, 1000, 60.0)]);
        let res = run(&r, Difficulty::Hard, 1000, |t| {
            Some(if within(t, 500.0, 520.0) { 79.0 } else { 60.0 })
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
            let m = within(t, 500.0, 1000.0).then_some(60.0);
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
    fn skipped_notes_are_excluded_not_zeroed() {
        let r = reference(&[(0, 500, 60.0), (1000, 500, 62.0), (2000, 500, 64.0)]);
        let mut s = Scorer::new(&r, 0, Difficulty::Normal);
        s.skip(600.0, 2000.0); // seek from 0.6 s to 2.0 s jumps over the 2nd note only
        for i in 0..300 {
            let t = i as f64 * 10.0;
            let m = if t < 500.0 {
                Some(60.0)
            } else if within(t, 2000.0, 2500.0) {
                Some(64.0)
            } else {
                None
            };
            s.push(VoiceFrame {
                t_ms: t,
                f0_hz: m.map(midi_hz),
                confidence: 0.9,
                voiced: m.is_some(),
            });
        }
        let res = s.finish();
        assert_eq!((res.score, res.notes_total, res.notes_hit), (100.0, 2, 2));
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
