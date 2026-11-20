//! Sung reference capture (T-005-07, ADR-0004 front A): the user sings the melody
//! once; analysis frames become notes. Parameters chosen from measurements
//! (docs/research/auto-reference-spike.md and the tests below).
//!
//! 1. Pitch per voiced frame: octave-aware 5-frame median (MIDI units).
//! 2. Voiced runs are split at ≥ 30 ms without voice (consonant, breath, rest).
//! 3. Inside a run, a Viterbi path over semitone states picks the notes: each
//!    frame costs its distance (semitones) to the state's pitch, each change of
//!    note costs `SWITCH_COST`. A vibrato around a note stays on it (its frames are
//!    nearer that note than the neighbours on average), a real change switches
//!    where the voice changes, and short wobbles are not worth a switch.
//! 4. Fragments one octave away from both neighbours are folded back
//!    (pitch-detector octave errors); notes shorter than 80 ms are dropped.
//! 5. Pitch = the state (nearest semitone, A440; see `Quantize` for the evidence).
//!
//! Output: sorted, non-overlapping, `dur_ms ≥ 1`, MIDI within 24–108, no lyrics.

use crate::Note;
use crate::VoiceFrame;
use crate::scorer::{fold_to, hz_to_midi, median};

const HOP_MS: f64 = 10.0;
/// ≥ this much time without a voiced frame ends a run (and the note).
const GAP_MS: f64 = 30.0;
/// Cost of changing note, in semitone·frames: a wobble must cost more than this
/// (e.g. 1 semitone off for 80 ms) to become a note of its own. Measured in
/// `tests/regression.rs` (t005_07): 2/4/8/12/16 → 8 keeps short passing notes
/// and halves the extra notes of a ±100 c vibrato (precision 56–70% → 82–83%).
const SWITCH_COST: f64 = 8.0;
const MIN_NOTE_MS: f64 = 80.0;
/// A neighbour closer than this (gap) can fold an octave fragment.
const OCTAVE_NEIGHBOUR_MS: f64 = 500.0;
const MIDI_MIN: i32 = 24;
const MIDI_MAX: i32 = 108;

/// How note pitches are written. Measured in `tests::quantization_evidence`:
/// rounding to the A440 semitone corrects what the person recording sang off
/// (constant or per-note errors up to ~40 c), which is the common case; it only
/// loses on videos tuned away from A440, which are rare.
// The alternatives exist for the measured comparison (tests).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Quantize {
    /// Nearest semitone, A440 (the Viterbi state).
    Semitone,
    /// Median as sung (fractional MIDI).
    Raw,
    /// Nearest semitone of a grid shifted by the duration-weighted global offset,
    /// when the offsets are concentrated (|mean resultant| ≥ 0.5).
    TuningGrid,
}

/// Notes sung in `frames` (song time, any order; frames before 0 ms are ignored).
pub fn segment_notes(frames: &[VoiceFrame]) -> Vec<Note> {
    segment_with(frames, Quantize::Semitone)
}

pub(crate) fn segment_with(frames: &[VoiceFrame], q: Quantize) -> Vec<Note> {
    let mut f: Vec<VoiceFrame> = frames
        .iter()
        .copied()
        .filter(|f| f.t_ms.is_finite() && f.t_ms >= 0.0)
        .collect();
    f.sort_by(|a, b| a.t_ms.total_cmp(&b.t_ms));
    let t: Vec<f64> = f.iter().map(|f| f.t_ms).collect();
    let raw: Vec<Option<f64>> = f
        .iter()
        .map(|f| {
            f.f0_hz
                .filter(|&hz| f.voiced && hz > 0.0)
                .map(|hz| hz_to_midi(hz as f64))
                .filter(|m| (MIDI_MIN as f64..=MIDI_MAX as f64).contains(m))
        })
        .collect();
    let m: Vec<Option<f64>> = (0..raw.len())
        .map(|i| {
            let own = raw[i]?;
            let lo = i.saturating_sub(2);
            let hi = (i + 3).min(raw.len());
            let mut w: Vec<f64> = raw[lo..hi]
                .iter()
                .flatten()
                .map(|&v| fold_to(v, own))
                .collect();
            median(&mut w)
        })
        .collect();

    // Voiced runs: maximal sets of voiced frames without a > GAP_MS hole.
    let voiced: Vec<usize> = (0..m.len()).filter(|&i| m[i].is_some()).collect();
    let mut notes: Vec<(f64, f64, f64)> = Vec::new(); // (start ms, end ms, pitch)
    let mut run: Vec<usize> = Vec::new();
    for (k, &i) in voiced.iter().enumerate() {
        run.push(i);
        if k + 1 == voiced.len() || t[voiced[k + 1]] - t[i] > GAP_MS {
            notes.extend(viterbi_notes(&run, &m, &t, q));
            run.clear();
        }
    }
    fold_octave_fragments(&mut notes);
    notes.retain(|&(s, e, _)| e - s >= MIN_NOTE_MS);
    if q == Quantize::TuningGrid {
        let offset = tuning_offset(&notes);
        notes
            .iter_mut()
            .for_each(|n| n.2 = (n.2 - offset).round() + offset);
    }
    let mut result: Vec<Note> = Vec::with_capacity(notes.len());
    for (s, e, p) in notes {
        if !(MIDI_MIN as f64..=MIDI_MAX as f64).contains(&p) {
            continue;
        }
        let mut start = s.max(0.0).round() as u32;
        if let Some(prev) = result.last() {
            start = start.max(prev.start_ms + prev.dur_ms);
        }
        let end = e.round() as u32;
        if end > start {
            result.push(Note {
                start_ms: start,
                dur_ms: end - start,
                midi: p as f32,
                lyric: None,
            });
        }
    }
    result
}

/// Index and value of the smallest element (first on ties).
fn argmin(v: &[f64]) -> (usize, f64) {
    v.iter().enumerate().fold(
        (0, f64::INFINITY),
        |a, (i, &c)| if c < a.1 { (i, c) } else { a },
    )
}

/// Notes of one voiced run (frame indices into `m`/`t`), pitch per `q`.
fn viterbi_notes(run: &[usize], m: &[Option<f64>], t: &[f64], q: Quantize) -> Vec<(f64, f64, f64)> {
    let mut vals: Vec<f64> = run.iter().filter_map(|&i| m[i]).collect();
    let Some(mid) = median(&mut vals) else {
        return Vec::new();
    };
    // States: the run's median pitch ± 2 octaves (wider than any phrase).
    let lo = mid.round() as i32 - 24;
    let states: Vec<f64> = (lo..=lo + 48).map(f64::from).collect();
    let mut cost = vec![0.0f64; states.len()];
    // came_from[k][s]: the state before s when the path switched into s at frame k.
    let mut came_from: Vec<Vec<Option<usize>>> = Vec::with_capacity(run.len());
    for &i in run {
        let v = m[i].unwrap_or(mid);
        let (best_s, best) = argmin(&cost);
        let mut row = vec![None; states.len()];
        for (s, c) in cost.iter_mut().enumerate() {
            if best + SWITCH_COST < *c {
                *c = best + SWITCH_COST;
                row[s] = Some(best_s);
            }
            *c += (v - states[s]).abs();
        }
        came_from.push(row);
    }
    let mut s = argmin(&cost).0;
    let mut path = vec![0usize; run.len()];
    for k in (0..run.len()).rev() {
        path[k] = s;
        if let Some(p) = came_from[k][s] {
            s = p;
        }
    }
    // A stretch shorter than MIN_NOTE_MS inside the run is a glitch: it keeps
    // the previous note's state (or the next one's at the start of the run).
    let mut a = 0;
    while a < run.len() {
        let mut b = a;
        while b < run.len() && path[b] == path[a] {
            b += 1;
        }
        let short = t[run[b - 1]] - t[run[a]] + HOP_MS < MIN_NOTE_MS;
        if short && (a > 0 || b < run.len()) {
            let fill = if a > 0 { path[a - 1] } else { path[b] };
            path[a..b].iter_mut().for_each(|p| *p = fill);
            a = 0; // neighbours may now join: rescan
            continue;
        }
        a = b;
    }
    let mut out = Vec::new();
    let mut a = 0;
    for k in 1..=run.len() {
        if k < run.len() && path[k] == path[a] {
            continue;
        }
        let state = states[path[a]];
        let pitch = match q {
            Quantize::Semitone => state,
            Quantize::Raw | Quantize::TuningGrid => {
                let mut p: Vec<f64> = run[a..k]
                    .iter()
                    .filter_map(|&i| m[i])
                    .map(|v| fold_to(v, state))
                    .collect();
                median(&mut p).unwrap_or(state)
            }
        };
        out.push((
            t[run[a]] - HOP_MS / 2.0,
            t[run[k - 1]] + HOP_MS / 2.0,
            pitch,
        ));
        a = k;
    }
    out
}

/// Pitch-detector octave errors: a note more than 9 semitones from every
/// neighbour within reach that lands within 7 of all of them one octave away is
/// moved there, when it has neighbours on both sides or is shorter than its only
/// neighbour (so a phrase's first note is not bent to a short fragment). The
/// score is octave-agnostic; this is for what the reference shows. Adjacent notes that
/// then share the pitch with no silence between are merged.
fn fold_octave_fragments(notes: &mut Vec<(f64, f64, f64)>) {
    for k in 0..notes.len() {
        let (s, e, p) = notes[k];
        let near: Vec<(f64, f64)> = [k.checked_sub(1), Some(k + 1)]
            .into_iter()
            .flatten()
            .filter_map(|j| notes.get(j))
            .filter(|n| (n.0 - e).max(s - n.1) < OCTAVE_NEIGHBOUR_MS)
            .map(|n| (n.2, n.1 - n.0))
            .collect();
        let Some(&(q0, d0)) = near.first() else {
            continue;
        };
        let folded = fold_to(p, q0);
        let octave_off = near
            .iter()
            .all(|&(q, _)| (p - q).abs() > 9.0 && (folded - q).abs() <= 7.0);
        if octave_off && (near.len() >= 2 || d0 > e - s) {
            notes[k].2 = folded;
        }
    }
    let mut merged: Vec<(f64, f64, f64)> = Vec::with_capacity(notes.len());
    for n in notes.drain(..) {
        match merged.last_mut() {
            Some(last) if (last.2 - n.2).abs() < 0.5 && n.0 - last.1 < HOP_MS => last.1 = n.1,
            _ => merged.push(n),
        }
    }
    *notes = merged;
}

/// Duration-weighted circular mean of the offsets from the semitone grid, in
/// semitones; 0 when they are spread out.
fn tuning_offset(notes: &[(f64, f64, f64)]) -> f64 {
    let (mut re, mut im, mut w) = (0.0, 0.0, 0.0);
    for &(s, e, p) in notes {
        let a = std::f64::consts::TAU * (p - p.round());
        re += (e - s) * a.cos();
        im += (e - s) * a.sin();
        w += e - s;
    }
    if w == 0.0 || (re * re + im * im).sqrt() / w < 0.5 {
        0.0
    } else {
        im.atan2(re) / std::f64::consts::TAU
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Difficulty, MelodyReference, Scorer, parse_reference_json, to_reference_json};
    use std::f64::consts::TAU;

    fn midi_hz(m: f64) -> f32 {
        (440.0 * 2f64.powf((m - 69.0) / 12.0)) as f32
    }

    /// Ideal 10 ms frames from a pitch function (None = unvoiced).
    fn frames(end_ms: f64, f: impl Fn(f64) -> Option<f64>) -> Vec<VoiceFrame> {
        (0..(end_ms / HOP_MS) as usize)
            .map(|i| {
                let t = i as f64 * HOP_MS;
                let m = f(t);
                VoiceFrame {
                    t_ms: t,
                    f0_hz: m.map(midi_hz),
                    confidence: 0.9,
                    voiced: m.is_some(),
                }
            })
            .collect()
    }

    fn melody() -> Vec<(f64, f64, f64)> {
        // (start, end, midi): legato steps, a repeated note after a consonant gap, a rest.
        vec![
            (500.0, 1000.0, 60.0),
            (1000.0, 1500.0, 62.0),
            (1500.0, 2000.0, 64.0),
            (2040.0, 2500.0, 64.0),
            (3000.0, 3800.0, 67.0),
            (3800.0, 4200.0, 65.0),
        ]
    }

    fn at(t: f64) -> Option<(f64, f64, f64)> {
        melody().into_iter().find(|&(s, e, _)| t >= s && t < e)
    }

    #[test]
    fn vibrato_scoop_and_consonants_give_the_sung_notes() {
        let fr = frames(5000.0, |t| {
            at(t).map(|(s, _, m)| {
                let scoop = -0.8 * (1.0 - ((t - s) / 60.0).min(1.0));
                m + scoop + 0.5 * (TAU * 5.5 * t / 1000.0).sin()
            })
        });
        let notes = segment_notes(&fr);
        let got: Vec<(u32, f32)> = notes.iter().map(|n| (n.start_ms, n.midi)).collect();
        let want: Vec<f32> = melody().iter().map(|n| n.2 as f32).collect();
        assert_eq!(
            got.iter().map(|g| g.1).collect::<Vec<_>>(),
            want,
            "{notes:?}"
        );
        for (n, w) in notes.iter().zip(melody()) {
            assert!((n.start_ms as f64 - w.0).abs() <= 30.0, "{n:?} vs {w:?}");
            assert!(
                ((n.start_ms + n.dur_ms) as f64 - w.1).abs() <= 30.0,
                "{n:?} vs {w:?}"
            );
        }
    }

    #[test]
    fn short_fragments_octave_errors_and_unvoiced_noise_are_cleaned() {
        let fr = frames(3000.0, |t| {
            if (500.0..1500.0).contains(&t) {
                // 50 ms glitch a fifth up, and 200 ms read one octave low.
                Some(if (700.0..750.0).contains(&t) {
                    67.0
                } else if (1000.0..1200.0).contains(&t) {
                    48.0
                } else {
                    60.0
                })
            } else if (2000.0..2050.0).contains(&t) {
                Some(72.0) // isolated 50 ms blip
            } else {
                None
            }
        });
        let notes = segment_notes(&fr);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0].midi, 60.0);
        assert!(notes[0].dur_ms >= 980, "{notes:?}");
    }

    #[test]
    fn a_short_chromatic_passing_note_survives() {
        let fr = frames(2000.0, |t| {
            if (500.0..1000.0).contains(&t) {
                Some(60.0)
            } else if (1000.0..1120.0).contains(&t) {
                Some(61.0)
            } else if (1120.0..1600.0).contains(&t) {
                Some(62.0)
            } else {
                None
            }
        });
        let midi: Vec<f32> = segment_notes(&fr).iter().map(|n| n.midi).collect();
        assert_eq!(midi, vec![60.0, 61.0, 62.0]);
    }

    #[test]
    fn output_is_a_valid_reference() {
        // Out-of-order frames, negative times, out-of-range pitch: still valid.
        let mut fr = frames(3000.0, |t| {
            if t < 1000.0 {
                Some(60.0)
            } else if t < 2000.0 {
                Some(115.0)
            } else {
                Some(64.0)
            }
        });
        fr.reverse();
        fr.push(VoiceFrame {
            t_ms: -50.0,
            f0_hz: Some(261.6),
            confidence: 0.9,
            voiced: true,
        });
        let notes = segment_notes(&fr);
        assert_eq!(
            notes.iter().map(|n| n.midi).collect::<Vec<_>>(),
            vec![60.0, 64.0]
        );
        let r = MelodyReference {
            format_version: crate::REFERENCE_FORMAT_VERSION,
            notes: notes.clone(),
        };
        let back = parse_reference_json(&to_reference_json(&r)).unwrap();
        assert_eq!(back.notes, notes);
        assert!(segment_notes(&[]).is_empty());
    }

    /// Evidence for `Quantize`: the person recording sings the test melody with an
    /// error; later, a singer follows the VIDEO perfectly. Score (Normal) of that
    /// singer against the recorded reference, per strategy.
    #[test]
    fn quantization_evidence() {
        let song: Vec<(f64, f64, f64)> = (0..24)
            .map(|i| {
                let s = 500.0 + i as f64 * 600.0;
                (
                    s,
                    s + 550.0,
                    [60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 71.0, 72.0][i % 8],
                )
            })
            .collect();
        let end = 500.0 + 24.0 * 600.0 + 500.0;
        // Per-note error pattern in cents (deterministic, zero mean).
        let jitter = |i: usize, amp: f64| amp * [0.9, -0.6, 0.3, -1.0, 0.7, -0.2, 0.5, -0.8][i % 8];
        let cases: [(&str, f64, f64, f64); 7] = [
            // (label, video tuning c, recorder constant bias c, recorder per-note error amplitude c)
            ("afinado", 0.0, 0.0, 0.0),
            ("gravador +20 c", 0.0, 20.0, 0.0),
            ("gravador +35 c", 0.0, 35.0, 0.0),
            ("gravador −30 c", 0.0, -30.0, 0.0),
            ("gravador ±25 c por nota", 0.0, 0.0, 25.0),
            ("gravador ±40 c por nota", 0.0, 0.0, 40.0),
            (
                "vídeo afinado +30 c, gravador segue o vídeo",
                30.0,
                0.0,
                0.0,
            ),
        ];
        let mut table = Vec::new();
        for (label, video, bias, amp) in cases {
            let rec = frames(end, |t| {
                song.iter()
                    .enumerate()
                    .find(|(_, n)| t >= n.0 && t < n.1)
                    .map(|(i, n)| {
                        n.2 + (video + bias + jitter(i, amp)) / 100.0
                            + 0.4 * (TAU * 5.5 * t / 1000.0).sin()
                    })
            });
            let singer = frames(end, |t| {
                song.iter()
                    .find(|n| t >= n.0 && t < n.1)
                    .map(|n| n.2 + video / 100.0)
            });
            let mut row = vec![label.to_owned()];
            for q in [Quantize::Semitone, Quantize::Raw, Quantize::TuningGrid] {
                let r = MelodyReference {
                    format_version: 1,
                    notes: segment_with(&rec, q),
                };
                let mut s = Scorer::new(&r, 0, Difficulty::Normal);
                singer.iter().for_each(|f| {
                    s.push(*f);
                });
                row.push(format!("{:.1}", s.finish().score));
            }
            table.push(row);
        }
        eprintln!("| Caso | Semitom (A440) | Fracionário | Grade com afinação global |");
        for r in &table {
            eprintln!("| {} |", r.join(" | "));
        }
        // The chosen strategy (Semitone) is best or tied in every recorder-error case
        // (all rows but the last, the video tuned away from A440).
        for row in &table[..table.len() - 1] {
            let score = |col: usize| row[col].parse::<f32>().unwrap();
            assert!(score(1) >= score(2) && score(1) >= score(3), "{row:?}");
        }
    }
}
