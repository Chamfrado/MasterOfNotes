//! Deterministic vocal scoring, algorithm v4 (SPEC-006 "Algoritmo v4", on top of
//! v3 and v2). Every constant below is quoted from the spec; changing any result
//! requires bumping `ALGORITHM_VERSION` and the golden values in
//! `tests/regression.rs`.
//!
//! Per note i (duration d, scored region = note minus transition zones), scored
//! once its frames are in:
//! - Pitch is judged on the vibrato-free contour: each voiced frame's in-note
//!   5-frame median, averaged over ~190 ms (one vibrato period) inside the note
//!   and between the same onsets (this note's and the next one's); values
//!   farther than 250 cents from the frame are not averaged in, so short
//!   ornaments stay local.
//! - Pitch error in cents (E1, v4):
//!   `|contour − target| + 2.5·max(0, spread − 60) + 2·max(0, movement − 20)`.
//!   Spread = mean `|median − contour|` in the window (≈ 62 for a ±100 c
//!   vibrato); movement = `|contour(+100 ms) − contour(−100 ms)|` (a vibrato's
//!   centre stays put). A trill or a wobble around the target is not singing it.
//! - `p(frame)` = 1 within T_full cents, 0 beyond T_zero, linear between
//!   (unvoiced = 0).
//! - `pitch_i` = Σp / frames that arrived (+ the frames after the last one
//!   received, which count 0). Frames that never arrived are left out (N2).
//! - `accuracy_i` = mean p over the voiced frames from the onset (or start +
//!   80 ms without one) until the next note's onset, so arriving late or
//!   leaving early is charged once, by rhythm and pitch (N3).
//! - `gate_i` = clamp(2·accuracy_i − 1) · min(1, 2·pitch_i).
//! - `rhythm_i` = timing(onset) · gate_i; `sustain_i` = held_i · gate_i.
//! - Notes shorter than 120 ms take the best of their region shifted by up to
//!   min(120 − d, 30) ms, in 10 ms steps, and their onset window ends that much
//!   later (N4: latency error on fast passages).
//!
//! Score = 100·(0.65·P + 0.20·R + 0.15·S), duration-weighted over all notes.

use crate::MelodyReference;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const ALGORITHM_VERSION: u32 = 4;

/// Frame period of the analysis (contract: one frame every 10 ms).
const HOP_MS: f64 = 10.0;
/// Median window (frames) applied to f0 before anything else: 5 = ±2.
const MEDIAN_HALF: usize = 2;
/// Contour window: 2·9 + 1 = 19 frames = 190 ms, about one period of a 5.3 Hz
/// vibrato. A boxcar of that length leaves ≤ 21% of a 4.5–7 Hz vibrato.
const CONTOUR_HALF: usize = 9;
/// Only values within this distance (semitones) of the frame enter its contour:
/// covers a ±125 cent vibrato, keeps note changes and ornaments out.
const CONTOUR_RANGE_ST: f64 = 2.5;
/// E1 (v4): oscillating around the target is not singing it. A natural vibrato
/// up to ±100 c spreads ≤ ~65 c around its centre (mean |frame − centre|); the
/// excess counts as pitch error, `SPREAD_WEIGHT` cents per cent.
const VIBRATO_SPREAD_CENTS: f64 = 60.0;
const SPREAD_WEIGHT: f64 = 2.5;
/// E1 (v4): a vibrato's centre stays put; an unstable voice's centre moves.
/// Movement of the centre between −100 and +100 ms counts as pitch error.
/// Movement up to `WANDER_FREE_CENTS` is free (slow drift, a 4 Hz vibrato's
/// residual); beyond it, `WANDER_WEIGHT` cents of error per cent.
const WANDER_FRAMES: usize = 10;
const WANDER_FREE_CENTS: f64 = 20.0;
const WANDER_WEIGHT: f64 = 2.0;
/// Onset search starts at max(start − 300 ms, end of the previous note + guard).
const ONSET_EARLY_MS: f64 = 300.0;
/// Right after a note ends the analysis window (≈ 43 ms, dated at its centre)
/// still contains it: half a window is release smear, never a new attack.
const ONSET_GUARD_MS: f64 = 25.0;
/// Rhythm ramp (symmetric): full credit ≤ 80 ms, zero ≥ 300 ms.
const RHYTHM_FULL_MS: f64 = 80.0;
const RHYTHM_ZERO_MS: f64 = 300.0;
/// A3 walk-back limit. Rhythm is already 0 at 300 ms, but the onset also splits
/// the contour (N3, E1), so it should land on the real change; the limit only
/// bounds the cost of a voice held through many notes.
const ONSET_BACK_MS: f64 = 1000.0;
/// Notes closer than this are "tied": the boundary gets a transition zone.
const TIED_GAP_MS: f64 = 30.0;
/// Transition zone length: min(25 ms, d/4).
const TRANSITION_MS: f64 = 25.0;
/// N4: notes shorter than this may be matched up to `SHORT_SHIFT_MS` off.
const SHORT_NOTE_MS: f64 = 120.0;
const SHORT_SHIFT_MS: f64 = 30.0;
/// A note is scored once frames reach its end + this (shift + median lookahead).
const SCORE_LAG_MS: f64 = SHORT_SHIFT_MS + (MEDIAN_HALF as f64 + 1.0) * HOP_MS;
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
/// Frames must come in increasing time; a frame not after the previous one is ignored.
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
    /// From the median error of the last 200 ms (≈ one vibrato period).
    pub label: FeedbackLabel,
    /// Score (0..100) over notes already scored (finished ≥ 60 ms ago).
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

#[derive(Debug, Clone, Copy)]
struct NoteScore {
    pitch: f64,
    rhythm: f64,
    sustain: f64,
}

impl NoteScore {
    fn weighted(&self) -> f64 {
        W_PITCH * self.pitch + W_RHYTHM * self.rhythm + W_SUSTAIN * self.sustain
    }
}

#[derive(Debug, Clone)]
struct NoteState {
    start: f64,
    end: f64,
    midi: f64,
    /// Scored region: the note minus transition zones at tied boundaries.
    region: (f64, f64),
    onset_from: f64,
    /// Target of the previous note: an onset must be nearer this note than that one.
    prev_midi: Option<f64>,
    /// Jumped over by a seek (training only): excluded from every aggregate.
    skipped: bool,
    /// Set once scored; `None` inside = no frame of the note arrived (excluded).
    score: Option<Option<NoteScore>>,
}

impl NoteState {
    fn dur(&self) -> f64 {
        self.end - self.start
    }
}

/// Frame counts of one note's region.
#[derive(Default)]
struct Tally {
    /// Frames that arrived, plus the ones expected after the last frame received.
    frames: u32,
    p_sum: f64,
    held: u32,
    acc_voiced: u32,
    acc_p_sum: f64,
}

/// Incremental scorer. Feed frames in time order with [`Scorer::push`]; call
/// [`Scorer::finish`] at the end. Same frames ⇒ same result.
pub struct Scorer {
    notes: Vec<NoteState>,
    tol: (f64, f64),
    raw: Vec<VoiceFrame>,
    next: usize,
    /// Live window: (time, octave-folded cents error of voiced frames).
    recent: VecDeque<(f64, Option<f64>)>,
    scored: usize,
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
                NoteState {
                    start,
                    end,
                    midi,
                    region: (
                        start + if tied_before { zone } else { 0.0 },
                        end - if tied_after { zone } else { 0.0 },
                    ),
                    onset_from: prev_end.map_or(start - ONSET_EARLY_MS, |p| {
                        (p + ONSET_GUARD_MS).min(start).max(start - ONSET_EARLY_MS)
                    }),
                    prev_midi: i.checked_sub(1).map(|j| spans[j].2),
                    skipped: false,
                    score: None,
                }
            })
            .collect();
        Self {
            notes,
            tol: difficulty.tolerances(),
            raw: Vec::new(),
            next: 0,
            recent: VecDeque::new(),
            scored: 0,
            done_dur: 0.0,
            done_weighted: 0.0,
        }
    }

    /// Returns feedback for the frame two positions back (centered median needs
    /// two frames of lookahead) when it falls inside a note.
    pub fn push(&mut self, frame: VoiceFrame) -> Option<LiveFeedback> {
        // ponytail: a backward re-anchor repeats song time; the repeat is dropped.
        if !frame.t_ms.is_finite() || self.raw.last().is_some_and(|l| frame.t_ms <= l.t_ms) {
            return None;
        }
        self.raw.push(frame);
        self.score_until(frame.t_ms - SCORE_LAG_MS, None);
        if self.raw.len() > self.next + MEDIAN_HALF {
            let fb = self.feedback(self.next);
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
        // Frames after the last one received were not sung (the run ended).
        let last = self.raw.last().map_or(f64::NEG_INFINITY, |f| f.t_ms);
        self.score_until(f64::INFINITY, Some(last));
        let notes: Vec<(f64, NoteScore)> = self
            .notes
            .iter()
            .filter(|n| !n.skipped)
            .filter_map(|n| n.score.flatten().map(|s| (n.dur(), s)))
            .collect();
        let total: f64 = notes.iter().map(|n| n.0).sum();
        let avg = |f: fn(&NoteScore) -> f64| {
            if total > 0.0 {
                notes.iter().map(|(d, s)| d * f(s)).sum::<f64>() / total
            } else {
                0.0
            }
        };
        let (p, r, s) = (avg(|s| s.pitch), avg(|s| s.rhythm), avg(|s| s.sustain));
        let hit = notes.iter().filter(|n| n.1.pitch >= 0.5).count() as u32;
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

    /// Scores every note ending at or before `t`. `last` (at finish) is the last
    /// frame received; frames expected after it count as unsung.
    fn score_until(&mut self, t: f64, last: Option<f64>) {
        while self.scored < self.notes.len() && self.notes[self.scored].end <= t {
            let j = self.scored;
            if !self.notes[j].skipped {
                let s = self.score_note(j, last);
                self.notes[j].score = Some(s);
                if let Some(s) = s {
                    self.done_dur += self.notes[j].dur();
                    self.done_weighted += self.notes[j].dur() * s.weighted();
                }
            }
            self.scored += 1;
        }
    }

    fn score_note(&self, j: usize, last: Option<f64>) -> Option<NoteScore> {
        let n = &self.notes[j];
        let onset = self.onset(j);
        let timing = onset.map_or(0.0, |o| {
            ramp((o - n.start).abs(), RHYTHM_FULL_MS, RHYTHM_ZERO_MS)
        });
        let acc_from = onset.unwrap_or(n.start + RHYTHM_FULL_MS);
        // Mirror of N3: moving on early is charged by the next note's rhythm, not here.
        let acc_to = (j + 1 < self.notes.len())
            .then(|| self.onset(j + 1))
            .flatten()
            .filter(|&o| o > acc_from)
            .unwrap_or(f64::INFINITY);
        let acc = (acc_from, acc_to);
        let steps = (short_reach(n.dur()) / HOP_MS).floor() as i32;
        // Shifts 0, −1, +1, −2, … steps: ties keep the smallest shift.
        let mut best: Option<NoteScore> = None;
        for k in (0..=steps).flat_map(|k| [-k, k]).skip(1) {
            let Some(c) = self.tally(j, k as f64 * HOP_MS, onset, acc, last) else {
                continue;
            };
            let denom = c.frames as f64;
            let pitch = c.p_sum / denom;
            let accuracy = if c.acc_voiced == 0 {
                0.0
            } else {
                c.acc_p_sum / c.acc_voiced as f64
            };
            let gate = (2.0 * accuracy - 1.0).clamp(0.0, 1.0) * (2.0 * pitch).min(1.0);
            let s = NoteScore {
                pitch,
                rhythm: timing * gate,
                sustain: c.held as f64 / denom * gate,
            };
            if best.is_none_or(|b| s.weighted() > b.weighted()) {
                best = Some(s);
            }
        }
        best
    }

    /// Frame counts of note `j` with its region shifted by `shift` ms; `None`
    /// when no frame of the region arrived and none is owed after the last one.
    fn tally(
        &self,
        j: usize,
        shift: f64,
        onset: Option<f64>,
        (acc_from, acc_to): (f64, f64),
        last: Option<f64>,
    ) -> Option<Tally> {
        let n = &self.notes[j];
        let (a, b) = (n.start + shift, n.end + shift);
        let (r0, r1) = (n.region.0 + shift, n.region.1 + shift);
        let lo = self.raw.partition_point(|f| f.t_ms < r0);
        let hi = self.raw.partition_point(|f| f.t_ms < r1);
        let m: Vec<Option<f64>> = (lo..hi).map(|i| self.smoothed_midi(i, a, b)).collect();
        // The contour never mixes frames across this note's onset (N3) or the
        // next note's onset (moving on early): one segment per sung pitch.
        let split_at = |t: f64| {
            Some(t)
                .filter(|&o| o > r0 && o < r1)
                .map(|o| self.raw[lo..hi].partition_point(|f| f.t_ms < o))
        };
        let split = onset.and_then(split_at).unwrap_or(0);
        let split2 = split_at(acc_to).unwrap_or(hi - lo).max(split);
        let mut c = Tally {
            frames: (hi - lo) as u32 + unreceived_after(r0, r1, last),
            ..Tally::default()
        };
        if c.frames == 0 {
            return None;
        }
        let segment = |k: usize| {
            if k < split {
                (0, split)
            } else if k < split2 {
                (split, split2)
            } else {
                (split2, m.len())
            }
        };
        // (centre, spread) of every voiced frame.
        let shape: Vec<Option<(f64, f64)>> = m
            .iter()
            .enumerate()
            .map(|(k, own)| {
                let (s0, s1) = segment(k);
                own.map(|own| contour(&m[s0..s1], k - s0, own))
            })
            .collect();
        for (k, sk) in shape.iter().enumerate() {
            let Some((centre, spread)) = *sk else {
                continue;
            };
            // E1: how far the centre moves in ±100 ms (a vibrato's centre stays put).
            // A jump beyond the contour range is a note change or an ornament,
            // already kept out of the contour: not wander.
            let (s0, s1) = segment(k);
            let around = |j: usize| shape[j.clamp(s0, s1 - 1)].map(|s| s.0);
            let wander = match (
                around(k.saturating_sub(WANDER_FRAMES)),
                around(k + WANDER_FRAMES),
            ) {
                (Some(x), Some(y)) => Some((fold_to(y, x) - x).abs())
                    .filter(|&d| d <= CONTOUR_RANGE_ST)
                    .map_or(0.0, |d| 100.0 * d),
                _ => 0.0,
            };
            let err = cents(centre, n.midi).abs()
                + SPREAD_WEIGHT * (100.0 * spread - VIBRATO_SPREAD_CENTS).max(0.0)
                + WANDER_WEIGHT * (wander - WANDER_FREE_CENTS).max(0.0);
            let p = pitch_credit(err, self.tol);
            c.p_sum += p;
            c.held += u32::from(p > 0.0);
            if (acc_from..acc_to).contains(&self.raw[lo + k].t_ms) {
                c.acc_voiced += 1;
                c.acc_p_sum += p;
            }
        }
        Some(c)
    }

    /// Onset of note `j`: first frame in `[onset_from, start + d/2 + reach]` on target
    /// (p ≥ 0.5) and nearer this target than the previous one. If the singer
    /// never gets nearer (e.g. constantly sharp towards the previous note), the
    /// first frame on target. If the voice is already on target when the window
    /// opens, the onset goes back to where that stretch began (A3), as long as
    /// the frames are strictly nearer this target than the previous one (≤ 1 s back).
    fn onset(&self, j: usize) -> Option<f64> {
        let n = &self.notes[j];
        let tol = self.tol;
        let on_target = |i: usize| {
            self.smoothed_midi(i, f64::NEG_INFINITY, f64::INFINITY)
                .filter(|&s| pitch_credit(cents(s, n.midi), tol) >= 0.5)
        };
        let nearer = |s: f64, strict: bool| {
            n.prev_midi.is_none_or(|pm| {
                let (this, prev) = (cents(s, n.midi).abs(), cents(s, pm).abs());
                if strict { this < prev } else { this <= prev }
            })
        };
        let lo = self.raw.partition_point(|f| f.t_ms < n.onset_from);
        let until = n.start + n.dur() / 2.0 + short_reach(n.dur());
        let hi = self.raw.partition_point(|f| f.t_ms <= until);
        let mut first = None;
        let mut found = None;
        for i in lo..hi {
            if let Some(s) = on_target(i) {
                first.get_or_insert(i);
                if nearer(s, false) {
                    found = Some(i);
                    break;
                }
            }
        }
        let mut i = found.or(first)?;
        if i == lo {
            while i > 0
                && self.raw[i - 1].t_ms >= n.start - ONSET_BACK_MS
                && on_target(i - 1).is_some_and(|s| nearer(s, true))
            {
                i -= 1;
            }
        }
        Some(self.raw[i].t_ms)
    }

    fn feedback(&mut self, i: usize) -> Option<LiveFeedback> {
        let t = self.raw[i].t_ms;
        // First note starting after t; the note containing t (if any) is the one before.
        let k = self.notes.partition_point(|n| n.start <= t);
        let j = k.checked_sub(1).filter(|&j| t < self.notes[j].end)?;
        let (start, end, target) = (self.notes[j].start, self.notes[j].end, self.notes[j].midi);
        // In-note median: neighbours' pitches never leak across a tied boundary.
        let sung = self.smoothed_midi(i, start, end);
        let err = sung.map(|s| cents(s, target));
        self.recent.retain(|&(rt, _)| rt > t - LIVE_WINDOW_MS);
        self.recent.push_back((t, err));
        Some(LiveFeedback {
            t_ms: t,
            target_midi: target as f32,
            sung_midi: sung.map(|s| s as f32),
            cents_error: err.map(|e| e as f32),
            label: self.label(),
            running_score: if self.done_dur > 0.0 {
                round1(100.0 * self.done_weighted / self.done_dur)
            } else {
                0.0
            },
        })
    }

    /// Median (in MIDI units) of the voiced f0 values in the centered 5-frame
    /// window, using only frames with time in `[from, to)`, each folded to this
    /// frame's octave first (pitch-detector octave jumps keep their pitch
    /// class); `None` if this frame itself is not voiced.
    fn smoothed_midi(&self, i: usize, from: f64, to: f64) -> Option<f64> {
        let own = self.raw[i];
        let own = hz_to_midi(own.f0_hz.filter(|&hz| own.voiced && hz > 0.0)? as f64);
        let lo = i.saturating_sub(MEDIAN_HALF);
        let hi = (i + MEDIAN_HALF).min(self.raw.len() - 1);
        let mut v: Vec<f64> = self.raw[lo..=hi]
            .iter()
            .filter(|f| f.voiced && f.t_ms >= from && f.t_ms < to)
            .filter_map(|f| f.f0_hz)
            .filter(|&hz| hz > 0.0)
            .map(|hz| fold_to(hz_to_midi(hz as f64), own))
            .collect();
        median(&mut v)
    }

    fn label(&self) -> FeedbackLabel {
        let n = self.recent.len();
        let mut errs: Vec<f64> = self.recent.iter().filter_map(|r| r.1).collect();
        let voiced = errs.len();
        let Some(e) = median(&mut errs).filter(|_| voiced * 2 >= n) else {
            return FeedbackLabel::NotDetected;
        };
        match pitch_credit(e, self.tol) * voiced as f64 / n as f64 {
            a if a >= 0.9 => FeedbackLabel::Excellent,
            a if a >= 0.75 => FeedbackLabel::VeryGood,
            a if a >= 0.5 => FeedbackLabel::Good,
            _ => FeedbackLabel::NeedsWork,
        }
    }
}

/// Vibrato-free pitch at position `k` of a segment of in-note medians: mean of
/// the voiced values in a 19-frame window kept inside the segment, each folded
/// to the octave of `own` and only if within `CONTOUR_RANGE_ST` of it. Also
/// returns the spread: mean |value − mean| of those values (semitones).
fn contour(seg: &[Option<f64>], k: usize, own: f64) -> (f64, f64) {
    let w = 2 * CONTOUR_HALF + 1;
    let lo = k
        .saturating_sub(CONTOUR_HALF)
        .min(seg.len().saturating_sub(w));
    let values: Vec<f64> = seg[lo..(lo + w).min(seg.len())]
        .iter()
        .flatten()
        .map(|&v| fold_to(v, own))
        .filter(|v| (v - own).abs() <= CONTOUR_RANGE_ST)
        .collect();
    let n = values.len() as f64; // ≥ 1: `own` is in its own window
    let mean = values.iter().sum::<f64>() / n;
    let spread = values.iter().map(|v| (v - mean).abs()).sum::<f64>() / n;
    (mean, spread)
}

/// N4: how far a short note's region (and onset window) may shift.
fn short_reach(dur: f64) -> f64 {
    (SHORT_NOTE_MS - dur).clamp(0.0, SHORT_SHIFT_MS)
}

/// Frames that would have arrived in `[r0, r1)` after the last one received
/// (10 ms grid continuing from `last`); 0 while the run is going on.
fn unreceived_after(r0: f64, r1: f64, last: Option<f64>) -> u32 {
    match last {
        None => 0,
        Some(l) if l >= r1 => 0,
        Some(l) if l < r0 => ((r1 - r0) / HOP_MS).floor() as u32,
        Some(l) => (((r1 - l) / HOP_MS).ceil() - 1.0).max(0.0) as u32,
    }
}

fn median(v: &mut [f64]) -> Option<f64> {
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

/// `midi` moved by whole octaves to within ±6 semitones of `near`.
fn fold_to(midi: f64, near: f64) -> f64 {
    midi - 12.0 * ((midi - near) / 12.0).round()
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
    use std::f64::consts::TAU;

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
        // its end (release guard). The singer enters note 2 at 750 ms; its window
        // opens at 825 with the voice already on target, so the onset goes back to
        // where the stretch began (v3 A3): 750 → err 250 → 50/220.
        let r2 = reference(&[(0, 800, 55.0), (1000, 1000, 60.0)]);
        let early = run(&r2, Difficulty::Normal, 2500, |t| {
            if t < 700.0 {
                Some(55.0)
            } else {
                within(t, 750.0, 2000.0).then_some(60.0)
            }
        });
        // Note 1 hears the early entry (750–800 ms, 500 cents off): it costs note 1
        // pitch, but its gate stops at note 2's onset (mirror of N3) → rhythm 1.
        let expected = (800.0 + 1000.0 * 50.0 / 220.0) / 1800.0;
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

    /// Pushes the frames of `[0, end_ms)` kept by `keep`, sung by `f`.
    fn run_with_gaps(
        r: &MelodyReference,
        end_ms: u32,
        keep: impl Fn(f64) -> bool,
        f: impl Fn(f64) -> Option<f64>,
    ) -> ScoreResult {
        let mut s = Scorer::new(r, 0, Difficulty::Normal);
        for t in (0..end_ms / 10)
            .map(|i| i as f64 * 10.0)
            .filter(|&t| keep(t))
        {
            let m = f(t);
            s.push(VoiceFrame {
                t_ms: t,
                f0_hz: m.map(midi_hz),
                confidence: 0.9,
                voiced: m.is_some(),
            });
        }
        s.finish()
    }

    #[test]
    fn missing_frames_are_left_out_but_silence_and_the_unsung_end_count() {
        let r = reference(&[(500, 1000, 60.0), (2000, 1000, 62.0)]);
        let sing = |t: f64| {
            if within(t, 500.0, 1500.0) {
                Some(60.0)
            } else {
                within(t, 2000.0, 3000.0).then_some(62.0)
            }
        };
        // N2: every 3rd frame never arrives (driver deficit): still perfect.
        let gappy = run_with_gaps(&r, 3500, |t| !((t / 10.0) as u32).is_multiple_of(3), sing);
        assert_eq!(gappy.score, 100.0, "{gappy:?}");
        // A whole note inside a capture gap is left out, like a skip.
        let hole = run_with_gaps(&r, 3500, |t| !within(t, 1800.0, 3200.0), sing);
        assert_eq!((hole.score, hole.notes_total), (100.0, 1), "{hole:?}");
        // Frames that arrive unvoiced still count 0.
        let half = run_with_gaps(&r, 3500, |_| true, |t| sing(t).filter(|_| t < 2500.0));
        assert!((half.pitch - 0.75).abs() < 0.01, "{half:?}");
        // The run stopping early is not "missing": the rest of the song is unsung.
        let stopped = run_with_gaps(&r, 3500, |t| t < 2100.0, sing);
        assert!((stopped.pitch - 0.55).abs() < 0.01, "{stopped:?}");
        assert_eq!(stopped.notes_total, 2);
    }

    #[test]
    fn early_entry_through_a_short_rest_goes_back_to_where_it_began() {
        // A3: rests of 100 ms; entering 250 ms early costs the same as 250 ms late.
        let r = reference(&[(500, 900, 55.0), (1500, 900, 60.0), (2500, 900, 64.0)]);
        let shifted = |delay: f64| {
            run(&r, Difficulty::Normal, 4000, |t| {
                r.notes
                    .iter()
                    .find(|n| {
                        let s = n.start_ms as f64 + delay;
                        within(t, s, s + n.dur_ms as f64)
                    })
                    .map(|n| n.midi as f64)
            })
        };
        let (early, late) = (shifted(-250.0), shifted(250.0));
        assert!(
            (early.rhythm - late.rhythm).abs() < 0.05,
            "{early:?} vs {late:?}"
        );
        assert!(early.rhythm < 0.3, "{early:?}");
    }

    #[test]
    fn holding_the_previous_note_is_charged_once() {
        // N3: tied notes; the singer holds the previous pitch 150 ms into each note,
        // versus staying silent for those 150 ms.
        let r = reference(&[(500, 500, 60.0), (1000, 500, 62.0), (1500, 500, 64.0)]);
        let target = |t: f64| {
            r.notes
                .iter()
                .position(|n| within(t, n.start_ms as f64, (n.start_ms + n.dur_ms) as f64))
        };
        let late = |hold: bool| {
            run(&r, Difficulty::Normal, 2500, |t| {
                let i = target(t)?;
                let start = r.notes[i].start_ms as f64;
                if i > 0 && t < start + 150.0 {
                    hold.then_some(r.notes[i - 1].midi as f64)
                } else {
                    Some(r.notes[i].midi as f64)
                }
            })
        };
        let (held, silent) = (late(true), late(false));
        assert!(
            (held.score - silent.score).abs() <= 1.0,
            "{held:?} vs {silent:?}"
        );
    }

    #[test]
    fn vibrato_is_not_detune() {
        // N1: ±100 cents at 5.5 Hz around the target scores like a straight tone;
        // the same vibrato around +100 cents is still a semitone off.
        let r = reference(&[(500, 1000, 60.0), (1700, 1000, 64.0), (2900, 1000, 67.0)]);
        let sing = |bias: f64| {
            run(&r, Difficulty::Normal, 4500, |t| {
                r.notes
                    .iter()
                    .find(|n| within(t, n.start_ms as f64, (n.start_ms + n.dur_ms) as f64))
                    .map(|n| n.midi as f64 + bias + (TAU * 5.5 * t / 1000.0).sin())
            })
        };
        assert!(sing(0.0).score >= 98.0, "{:?}", sing(0.0));
        assert!(sing(1.0).score <= 40.0, "{:?}", sing(1.0));
    }

    #[test]
    fn oscillating_around_the_target_is_not_singing_it() {
        // E1 (v4): a square trill between the two neighbouring semitones has its
        // mean on the target but never sings it; a slow ±100 c swing moves its centre.
        let r = reference(&[(500, 1000, 60.0), (1700, 1000, 64.0), (2900, 1000, 67.0)]);
        let sing = |dev: &dyn Fn(f64) -> f64| {
            run(&r, Difficulty::Normal, 4500, |t| {
                r.notes
                    .iter()
                    .find(|n| within(t, n.start_ms as f64, (n.start_ms + n.dur_ms) as f64))
                    .map(|n| n.midi as f64 + dev(t))
            })
        };
        let trill = sing(&|t| {
            if ((t / 1000.0) * 12.0) as i64 % 2 == 0 {
                -1.0
            } else {
                1.0
            }
        });
        assert!(trill.score <= 50.0, "{trill:?}");
        let slow = sing(&|t| (TAU * 2.0 * t / 1000.0).sin());
        assert!(slow.score <= 50.0, "{slow:?}");
        let vibrato = sing(&|t| (TAU * 5.5 * t / 1000.0).sin());
        assert!(vibrato.score >= 98.0, "{vibrato:?}");
    }

    #[test]
    fn short_notes_tolerate_a_small_timing_error() {
        // N4: tied 50 ms notes sung 20 ms late.
        let notes: Vec<(u32, u32, f32)> = (0..20)
            .map(|i| (500 + i * 50, 50, if i % 2 == 0 { 60.0 } else { 64.0 }))
            .collect();
        let r = reference(&notes);
        let res = run(&r, Difficulty::Normal, 2000, |t| {
            let i = ((t - 520.0) / 50.0).floor();
            (0.0..20.0)
                .contains(&i)
                .then_some(if (i as u32).is_multiple_of(2) {
                    60.0
                } else {
                    64.0
                })
        });
        assert!(res.score >= 90.0, "{res:?}");
    }

    #[test]
    fn frames_must_advance_in_time() {
        let r = reference(&[(0, 500, 60.0)]);
        let mut s = Scorer::new(&r, 0, Difficulty::Normal);
        for t in [0.0, 10.0, 10.0, 5.0, f64::NAN, 20.0] {
            s.push(VoiceFrame {
                t_ms: t,
                f0_hz: Some(midi_hz(60.0)),
                confidence: 0.9,
                voiced: true,
            });
        }
        assert_eq!(s.raw.len(), 3);
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
