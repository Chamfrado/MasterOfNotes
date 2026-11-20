//! Reference ↔ video offset from chroma (SPEC-012 R5).
//!
//! The reference notes give an ideal chroma (one pitch class per frame); the
//! mix gives a measured one (STFT magnitudes folded onto 12 pitch classes,
//! mean-removed and unit-normalised per frame, so drums and silence weigh 0).
//! The offset is the lag that maximises their mean agreement over ±15 s,
//! after removing the slow part of that curve (see `DETREND_MS`).
//!
//! The spike aligned two *recordings* with DTW (tempo could differ). Here the
//! correction is a single constant offset, so the exhaustive lag search is the
//! exact optimum of the same chroma match under that constraint, and its peak
//! shape gives the confidence. Evidence: docs/testing/auto-reference.md.

use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;
use scoring_engine::Note;

/// Analysis hop (ms).
const HOP_MS: f64 = 10.0;
/// Analysis window ≈ 186 ms (enough resolution for semitones from ~110 Hz).
const WINDOW_S: f64 = 0.186;
/// Offsets searched: ±15 s.
const MAX_LAG_MS: f64 = 15_000.0;
/// Pitch range folded into the chroma (voice and its first harmonics).
const MIN_HZ: f64 = 80.0;
const MAX_HZ: f64 = 5_000.0;
/// A rival peak must be this far from the best to count as an alternative.
const RIVAL_MS: f64 = 1_000.0;
/// Half-width of the moving average removed from the lag scores.
const DETREND_MS: f64 = 500.0;

/// Below this the proposal is not reliable (no audible melody, or the
/// arrangement repeats): voice present gave 0.82–0.90, karaoke without a
/// melody and unrelated audio stayed below it (docs/testing/auto-reference.md).
pub const LOW_CONFIDENCE: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlignResult {
    /// Add to the reference's current offset.
    pub offset_ms: i32,
    /// 0..1: (peak − best rival more than 1 s away) ÷ peak; 0 = no peak or a tie.
    pub confidence: f32,
}

/// Offset (ms) that moves `notes` onto the melody/harmony of `mix`.
pub fn align_offset_ms(notes: &[Note], mix: &[f32], sample_rate: u32) -> AlignResult {
    const NONE: AlignResult = AlignResult {
        offset_ms: 0,
        confidence: 0.0,
    };
    let sr = sample_rate as f64;
    let hop = (sr * HOP_MS / 1000.0).round() as usize;
    let n_fft = (sr * WINDOW_S).round() as usize;
    if hop == 0 || mix.len() < n_fft || notes.is_empty() {
        return NONE;
    }
    let chroma = mix_chroma(mix, sr, n_fft, hop);
    // Reference chroma on the same frame grid: frame u is centred at
    // (u·hop + n_fft/2) / sr.
    let centre_ms = |u: usize| (u * hop + n_fft / 2) as f64 * 1000.0 / sr;
    let mut sorted: Vec<&Note> = notes.iter().collect();
    sorted.sort_by_key(|n| n.start_ms);
    let mut marks: Vec<(isize, usize)> = Vec::new(); // (frame, pitch class)
    let mut k = 0;
    let last_end = sorted
        .iter()
        .map(|n| (n.start_ms + n.dur_ms) as f64)
        .fold(0.0, f64::max);
    let frames_needed = ((last_end * sr / 1000.0) as usize).div_ceil(hop) + 1;
    for u in 0..frames_needed {
        let t = centre_ms(u);
        while k < sorted.len() && (sorted[k].start_ms + sorted[k].dur_ms) as f64 <= t {
            k += 1;
        }
        if let Some(n) = sorted.get(k).filter(|n| n.start_ms as f64 <= t) {
            marks.push((u as isize, pitch_class(n.midi as f64)));
        }
    }
    if marks.is_empty() {
        return NONE;
    }
    let max_lag = (MAX_LAG_MS / HOP_MS) as isize;
    let scores: Vec<f64> = (-max_lag..=max_lag)
        .map(|lag| {
            let (mut sum, mut valid) = (0.0, 0usize);
            for &(u, pc) in &marks {
                if let Some(c) = usize::try_from(u + lag).ok().and_then(|i| chroma.get(i)) {
                    sum += c[pc] as f64;
                    valid += 1;
                }
            }
            // A lag that leaves most of the notes outside the video is no match.
            if valid * 2 < marks.len() {
                f64::NEG_INFINITY
            } else {
                sum / valid as f64
            }
        })
        .collect();
    // Sustained harmony (chords lasting seconds) adds a broad bump whose slope
    // drags the peak; minus its ±500 ms moving average, only the note-level
    // match remains (a linear trend minus its symmetric mean is 0).
    let half = (DETREND_MS / HOP_MS) as usize;
    let scores: Vec<f64> = (0..scores.len())
        .map(|i| {
            if !scores[i].is_finite() {
                return f64::NEG_INFINITY;
            }
            let w = &scores[i.saturating_sub(half)..(i + half + 1).min(scores.len())];
            let (sum, n) = w
                .iter()
                .filter(|s| s.is_finite())
                .fold((0.0, 0usize), |(a, n), s| (a + s, n + 1));
            scores[i] - sum / n as f64
        })
        .collect();
    let Some((best, &peak)) = scores.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) else {
        return NONE;
    };
    if peak <= 1e-9 {
        return NONE;
    }
    // Sub-frame peak (parabola through the neighbours).
    let frac = match (scores.get(best.wrapping_sub(1)), scores.get(best + 1)) {
        (Some(&a), Some(&c)) if a.is_finite() && c.is_finite() => {
            let d = a - 2.0 * peak + c;
            if d < 0.0 { 0.5 * (a - c) / d } else { 0.0 }
        }
        _ => 0.0,
    };
    let lag = best as f64 - max_lag as f64 + frac;
    let rival_frames = (RIVAL_MS / HOP_MS) as usize;
    let rival = scores
        .iter()
        .enumerate()
        .filter(|(i, _)| i.abs_diff(best) > rival_frames)
        .map(|(_, &s)| s)
        .fold(0.0, f64::max);
    let confidence = ((peak - rival) / peak).clamp(0.0, 1.0);
    AlignResult {
        offset_ms: (lag * HOP_MS).round() as i32,
        confidence: confidence as f32,
    }
}

fn pitch_class(midi: f64) -> usize {
    (midi.round() as i64).rem_euclid(12) as usize
}

/// Per-frame chroma of `mix`: mean-removed, unit L2 norm (zeros when flat).
fn mix_chroma(mix: &[f32], sr: f64, n_fft: usize, hop: usize) -> Vec<[f32; 12]> {
    let fft = FftPlanner::<f32>::new().plan_fft_forward(n_fft);
    let window: Vec<f32> = (0..n_fft)
        .map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n_fft as f64).cos()) as f32)
        .collect();
    let bins: Vec<(usize, usize)> = (1..n_fft / 2)
        .filter_map(|k| {
            let hz = k as f64 * sr / n_fft as f64;
            (MIN_HZ..=MAX_HZ)
                .contains(&hz)
                .then(|| (k, pitch_class(69.0 + 12.0 * (hz / 440.0).log2())))
        })
        .collect();
    let mut buf = vec![Complex32::default(); n_fft];
    let frames = (mix.len() - n_fft) / hop + 1;
    (0..frames)
        .map(|u| {
            let seg = &mix[u * hop..u * hop + n_fft];
            for ((b, &s), &w) in buf.iter_mut().zip(seg).zip(&window) {
                *b = Complex32::new(s * w, 0.0);
            }
            fft.process(&mut buf);
            let mut c = [0.0f32; 12];
            for &(k, pc) in &bins {
                c[pc] += buf[k].norm();
            }
            let mean = c.iter().sum::<f32>() / 12.0;
            c.iter_mut().for_each(|v| *v -= mean);
            let norm = c.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 1e-6 {
                c.iter_mut().for_each(|v| *v /= norm);
            } else {
                c = [0.0; 12];
            }
            c
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    const SR: u32 = 22_050;

    /// xorshift64 in [0, 1).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// A 40 s song: a chord every 2 s (I–vi–IV–V in C) as piano-like
    /// harmonic tones, a melody mostly on chord tones (plus passing notes),
    /// a kick/snare noise pattern. Returns (melody notes, accompaniment, voice).
    fn song() -> (Vec<Note>, Vec<f32>, Vec<f32>) {
        let secs = 40.0;
        let n = (secs * SR as f64) as usize;
        let chords: [[f64; 3]; 4] = [
            [48.0, 52.0, 55.0],
            [45.0, 48.0, 52.0],
            [41.0, 45.0, 48.0],
            [43.0, 47.0, 50.0],
        ];
        let mut rng = Rng(0x5EED_1234_ABCD);
        let mut notes = Vec::new();
        let mut t = 1500.0;
        while t < secs * 1000.0 - 1000.0 {
            let chord = chords[((t / 2000.0) as usize) % 4];
            let dur = 200.0 + 400.0 * rng.next();
            let midi = if rng.next() < 0.8 {
                chord[(rng.next() * 3.0) as usize]
                    + 12.0
                    + if rng.next() < 0.3 { 12.0 } else { 0.0 }
            } else {
                [60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 71.0][(rng.next() * 7.0) as usize]
            };
            notes.push(Note {
                start_ms: t as u32,
                dur_ms: dur as u32,
                midi: midi as f32,
                lyric: None,
            });
            t += dur + if rng.next() < 0.3 { 150.0 } else { 20.0 };
        }
        let tone = |hz: f64, i: usize| -> f64 {
            (1..=4)
                .map(|h| (TAU * hz * h as f64 * i as f64 / SR as f64).sin() / h as f64)
                .sum()
        };
        let hz = |m: f64| 440.0 * 2f64.powf((m - 69.0) / 12.0);
        let mut acc = vec![0.0f32; n];
        let mut noise = Rng(99);
        for (i, s) in acc.iter_mut().enumerate() {
            let ms = i as f64 * 1000.0 / SR as f64;
            let chord = chords[((ms / 2000.0) as usize) % 4];
            let decay = (-((ms % 500.0) / 400.0)).exp();
            let harm: f64 =
                chord.iter().map(|&m| tone(hz(m), i)).sum::<f64>() * 0.08 * (0.4 + 0.6 * decay);
            let beat = ms % 500.0;
            let drum = if beat < 60.0 {
                (noise.next() - 0.5) * 0.5 * (1.0 - beat / 60.0)
            } else {
                0.0
            };
            *s = (harm + drum) as f32;
        }
        let mut voice = vec![0.0f32; n];
        for note in &notes {
            let a = (note.start_ms as f64 * SR as f64 / 1000.0) as usize;
            let b = (((note.start_ms + note.dur_ms) as f64) * SR as f64 / 1000.0) as usize;
            let mut phase = 0.0;
            for (i, s) in voice.iter_mut().enumerate().take(b.min(n)).skip(a) {
                let x = (i - a) as f64 / SR as f64;
                let vib = 0.3 * (TAU * 5.5 * x).sin() * (x / 0.3).min(1.0);
                phase += TAU * hz(note.midi as f64 + vib) / SR as f64;
                *s += (0.25
                    * (1..=4)
                        .map(|h| (h as f64 * phase).sin() / h as f64)
                        .sum::<f64>()) as f32;
            }
        }
        (notes, acc, voice)
    }

    fn shifted(notes: &[Note], by_ms: i64) -> Vec<Note> {
        notes
            .iter()
            .filter_map(|n| {
                let s = n.start_ms as i64 + by_ms;
                (s >= 0).then(|| Note {
                    start_ms: s as u32,
                    ..n.clone()
                })
            })
            .collect()
    }

    #[test]
    fn midi_shifted_by_1_2_s_is_proposed_within_30_ms() {
        let (notes, acc, voice) = song();
        let mix: Vec<f32> = acc.iter().zip(&voice).map(|(a, v)| a + v).collect();
        for (shift, label) in [(-1200i64, "com voz"), (1200, "com voz"), (-1234, "com voz")] {
            let r = align_offset_ms(&shifted(&notes, shift), &mix, SR);
            eprintln!(
                "| {label}, MIDI {shift:+} ms | proposta {:+} ms | confiança {:.2} |",
                r.offset_ms, r.confidence
            );
            assert!((r.offset_ms as i64 + shift).abs() <= 30, "{shift}: {r:?}");
            assert!(r.confidence >= 0.5, "{shift}: {r:?}");
        }
        let r = align_offset_ms(&shifted(&notes, -1200), &voice, SR);
        eprintln!(
            "| só a voz, MIDI -1200 ms | proposta {:+} ms | confiança {:.2} |",
            r.offset_ms, r.confidence
        );
        assert!(
            (r.offset_ms - 1200).abs() <= 30 && r.confidence >= 0.5,
            "{r:?}"
        );
    }

    #[test]
    fn karaoke_without_a_melody_is_not_confident() {
        // Accompaniment only (no guide voice): the chord loop repeats every 8 s,
        // so several offsets fit equally; the answer must say so.
        let (notes, acc, _) = song();
        for shift in [-1200i64, 0, 1200] {
            let r = align_offset_ms(&shifted(&notes, shift), &acc, SR);
            eprintln!(
                "| só acompanhamento, MIDI {shift:+} ms | proposta {:+} ms | confiança {:.2} |",
                r.offset_ms, r.confidence
            );
            assert!(
                r.confidence < LOW_CONFIDENCE || (r.offset_ms as i64 + shift).abs() <= 30,
                "{shift}: {r:?}"
            );
        }
    }

    #[test]
    fn unrelated_audio_gives_low_confidence() {
        let (notes, ..) = song();
        let mut rng = Rng(7);
        let noise: Vec<f32> = (0..40 * SR as usize)
            .map(|_| (rng.next() - 0.5) as f32 * 0.3)
            .collect();
        let r = align_offset_ms(&notes, &noise, SR);
        eprintln!(
            "| ruído | proposta {:+} ms | confiança {:.2} |",
            r.offset_ms, r.confidence
        );
        assert!(r.confidence < LOW_CONFIDENCE, "{r:?}");
        let silent = align_offset_ms(&notes, &vec![0.0; 40 * SR as usize], SR);
        assert_eq!(
            silent,
            AlignResult {
                offset_ms: 0,
                confidence: 0.0
            }
        );
        assert_eq!(align_offset_ms(&[], &noise, SR).confidence, 0.0);
    }
}
