//! SPEC-006 acceptance table, run through the full pipeline:
//! synthetic audio → audio_engine::analyze_buffer → Scorer (Normal difficulty).
//!
//! Fixture song: 10 notes of 1000 ms separated by 200 ms rests, starting at 500 ms,
//! melody C4 D4 E4 F4 G4 A4 G4 F4 E4 D4. All audio is generated in code.
//! Run `cargo test -p scoring-engine --test regression -- --nocapture --test-threads=1`
//! to print the measured numbers (copied into docs/testing/scoring-accuracy.md).

use audio_engine::{CaptureConfig, analyze_buffer, window_len};
use scoring_engine::{Difficulty, MelodyReference, Note, ScoreResult, Scorer, VoiceFrame};
use std::f64::consts::TAU;

const SR: u32 = 48_000;
const MELODY: [f64; 10] = [60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 67.0, 65.0, 64.0, 62.0];
const LEAD_MS: f64 = 500.0;
const NOTE_MS: f64 = 1000.0;
const GAP_MS: f64 = 200.0;
const TAIL_MS: f64 = 700.0;
/// Voice peak amplitude (sine RMS ≈ −17 dBFS).
const AMP: f64 = 0.2;

fn note_start(i: usize) -> f64 {
    LEAD_MS + i as f64 * (NOTE_MS + GAP_MS)
}

fn song_ms() -> f64 {
    note_start(MELODY.len()) + TAIL_MS
}

fn reference() -> MelodyReference {
    MelodyReference {
        format_version: 1,
        notes: MELODY
            .iter()
            .enumerate()
            .map(|(i, &m)| Note {
                start_ms: note_start(i) as u32,
                dur_ms: NOTE_MS as u32,
                midi: m as f32,
                lyric: None,
            })
            .collect(),
    }
}

/// Index of the note sounding at `t` (ms) in the reference timing, shifted by `delay`.
fn note_at(t: f64, delay: f64) -> Option<usize> {
    (0..MELODY.len())
        .find(|&i| (note_start(i) + delay..note_start(i) + delay + NOTE_MS).contains(&t))
}

fn midi_hz(m: f64) -> f64 {
    440.0 * 2f64.powf((m - 69.0) / 12.0)
}

/// Renders a tone whose pitch (MIDI, may be fractional) at time t (ms) is `pitch(t)`;
/// `None` = silence. Continuous phase, so glides and vibrato have no clicks.
/// `harmonics` = 1 for a sine, >1 for a voice-like 1/k spectrum.
fn render(pitch: impl Fn(f64) -> Option<f64>, amp: f64, harmonics: u32) -> Vec<f32> {
    let n = (song_ms() / 1000.0 * SR as f64) as usize;
    let norm: f64 = (1..=harmonics).map(|k| 1.0 / k as f64).sum();
    let mut phase = 0.0f64;
    (0..n)
        .map(|i| {
            let t = i as f64 * 1000.0 / SR as f64;
            match pitch(t) {
                Some(m) => {
                    phase = (phase + TAU * midi_hz(m) / SR as f64) % (TAU * 1e6);
                    let s: f64 = (1..=harmonics)
                        .map(|k| (phase * k as f64).sin() / k as f64)
                        .sum();
                    (amp * s / norm) as f32
                }
                None => {
                    phase = 0.0;
                    0.0
                }
            }
        })
        .collect()
}

fn sung(t: f64) -> Option<f64> {
    note_at(t, 0.0).map(|i| MELODY[i])
}

fn score(samples: &[f32]) -> ScoreResult {
    let frames = analyze_buffer(samples, SR, &CaptureConfig::default());
    let centre = window_len(SR) as f64 / 2.0;
    let mut scorer = Scorer::new(&reference(), 0, Difficulty::Normal);
    for f in frames {
        scorer.push(VoiceFrame {
            t_ms: (f.sample_index as f64 + centre) * 1000.0 / SR as f64,
            f0_hz: f.f0_hz,
            confidence: f.confidence,
            voiced: f.voiced,
        });
    }
    scorer.finish()
}

fn report(case: &str, r: &ScoreResult) {
    eprintln!(
        "| {case} | {:.1} | {:.3} | {:.3} | {:.3} | {}/{} |",
        r.score, r.pitch, r.rhythm, r.sustain, r.notes_hit, r.notes_total
    );
}

fn perfect() -> Vec<f32> {
    render(sung, AMP, 1)
}

#[test]
fn perfect_sine_scores_at_least_98() {
    let r = score(&perfect());
    report("Perfeito (senoide)", &r);
    assert!(r.score >= 98.0, "{r:?}");
    assert_eq!(r.notes_hit, 10);
    assert_eq!(r.algorithm_version, 1);
}

#[test]
fn perfect_voice_like_tone_scores_at_least_98() {
    let r = score(&render(sung, AMP, 8));
    report("Perfeito (voz harmônica, 8 harmônicos) — extra", &r);
    assert!(r.score >= 98.0, "{r:?}");
}

#[test]
fn perfect_one_octave_below_scores_at_least_98() {
    let r = score(&render(|t| sung(t).map(|m| m - 12.0), AMP, 1));
    report("Perfeito uma oitava abaixo", &r);
    assert!(r.score >= 98.0, "{r:?}");
}

#[test]
fn plus_100_cents_gives_half_pitch() {
    // Exactly on the p = 0.5 boundary: sustain/onset (p ≥ 0.5) are decided by
    // sub-cent analysis error, so only P is asserted, as the spec does.
    let r = score(&render(|t| sung(t).map(|m| m + 1.0), AMP, 1));
    report("+100 cents constante", &r);
    assert!((0.45..=0.55).contains(&r.pitch), "{r:?}");
}

#[test]
fn plus_200_cents_scores_zero_pitch() {
    let r = score(&render(|t| sung(t).map(|m| m + 2.0), AMP, 1));
    report("+200 cents constante", &r);
    assert_eq!(r.pitch, 0.0, "{r:?}");
    assert!(r.score <= 25.0, "{r:?}");
}

#[test]
fn silence_scores_zero() {
    let r = score(&render(|_| None, AMP, 1));
    report("Silêncio", &r);
    assert_eq!(r.score, 0.0, "{r:?}");
}

#[test]
fn white_noise_scores_at_most_5() {
    let n = (song_ms() / 1000.0 * SR as f64) as usize;
    let mut seed = 0x5EED_u64;
    let noise: Vec<f32> = (0..n)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            0.3 * (2.0 * ((seed >> 11) as f32 / (1u64 << 53) as f32) - 1.0)
        })
        .collect();
    let r = score(&noise);
    report("Ruído branco", &r);
    assert!(r.score <= 5.0, "{r:?}");
}

#[test]
fn delay_of_200ms_costs_rhythm_not_pitch() {
    let r = score(&render(|t| note_at(t, 200.0).map(|i| MELODY[i]), AMP, 1));
    report("Atraso de 200 ms", &r);
    assert!((0.3..=0.6).contains(&r.rhythm), "{r:?}");
    assert!(r.pitch >= 0.7, "{r:?}");
}

#[test]
fn one_of_ten_notes_scores_at_most_15() {
    let r = score(&render(
        |t| sung(t).filter(|_| note_at(t, 0.0) == Some(0)),
        AMP,
        1,
    ));
    report("Só 1 de 10 notas cantada", &r);
    assert!(r.score <= 15.0, "{r:?}");
    assert_eq!(r.notes_hit, 1);
}

#[test]
fn four_times_louder_is_identical() {
    let base = perfect();
    let loud: Vec<f32> = base.iter().map(|s| s * 4.0).collect();
    let (a, b) = (score(&base), score(&loud));
    report("Perfeito com volume 4×", &b);
    assert_eq!(a, b);
}

#[test]
fn vibrato_50_cents_6hz_scores_at_least_90() {
    let r = score(&render(
        |t| sung(t).map(|m| m + 0.5 * (TAU * 6.0 * t / 1000.0).sin()),
        AMP,
        1,
    ));
    report("Vibrato ±50 cents a 6 Hz", &r);
    assert!(r.score >= 90.0, "{r:?}");
}

#[test]
fn voice_with_accompaniment_at_minus_12db_scores_at_least_85() {
    // Voice: perfect voice-like tone. Accompaniment: major triad one octave below the
    // current note (root, third, fifth), held through the following rest, total RMS
    // 12 dB below the voice.
    let voice = render(sung, AMP, 8);
    let voice_rms = (voice.iter().map(|&s| (s as f64).powi(2)).sum::<f64>()
        / voice.iter().filter(|s| **s != 0.0).count() as f64)
        .sqrt();
    let chord_amp = voice_rms * 10f64.powf(-12.0 / 20.0) / (3.0f64 / 2.0).sqrt();
    let chord_note = |t: f64| {
        (0..MELODY.len())
            .rev()
            .find(|&i| t >= note_start(i))
            .map(|i| MELODY[i] - 12.0)
    };
    let parts: Vec<Vec<f32>> = [0.0, 4.0, 7.0]
        .iter()
        .map(|&iv| render(|t| chord_note(t).map(|m| m + iv), chord_amp, 1))
        .collect();
    let mix: Vec<f32> = (0..voice.len())
        .map(|i| voice[i] + parts.iter().map(|p| p[i]).sum::<f32>())
        .collect();
    let r = score(&mix);
    report("Voz + acompanhamento a −12 dB", &r);
    assert!(r.score >= 85.0, "{r:?}");
}

#[test]
fn glissando_of_80ms_into_each_note_scores_at_least_92() {
    // Each note starts on the previous note's pitch and slides (linear in MIDI) to
    // the target over its first 80 ms.
    let r = score(&render(
        |t| {
            note_at(t, 0.0).map(|i| {
                let from = if i == 0 { MELODY[0] } else { MELODY[i - 1] };
                let x = ((t - note_start(i)) / 80.0).min(1.0);
                from + (MELODY[i] - from) * x
            })
        },
        AMP,
        1,
    ));
    report("Perfeito com glissando de 80 ms entre notas", &r);
    assert!(r.score >= 92.0, "{r:?}");
}

#[test]
fn scoring_is_deterministic() {
    let s = render(
        |t| sung(t).map(|m| m + 0.3 * (TAU * 5.0 * t / 1000.0).sin()),
        AMP,
        4,
    );
    assert_eq!(score(&s), score(&s));
}
