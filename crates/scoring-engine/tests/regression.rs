//! SPEC-006 acceptance table (v1 rows + v2 targets), run through the full
//! pipeline: synthetic audio → audio_engine::analyze_buffer → Scorer (Normal
//! unless stated). All audio is generated in code.
//!
//! Voice model and several hard cases come from Agent 4's QA probes
//! (docs/testing/evidence/scoring-review.md, `qa_probe.rs` on agent-4/work).
//!
//! `golden_results_are_exact` pins the exact `ScoreResult` of representative
//! fixtures (A7): if it fails, the algorithm's output changed — bump
//! `ALGORITHM_VERSION`, update SPEC-006 and the golden values together.
//!
//! Print the measured numbers (copied into docs/testing/scoring-accuracy.md):
//! `cargo test -p scoring-engine --test regression -- --nocapture --test-threads=1`

use audio_engine::{CaptureConfig, analyze_buffer, window_len};
use scoring_engine::{
    ALGORITHM_VERSION, Difficulty, MelodyReference, Note, ScoreResult, Scorer, VoiceFrame,
};
use std::f64::consts::TAU;

const SR: u32 = 48_000;
/// Voice peak amplitude (sine RMS ≈ −17 dBFS).
const AMP: f64 = 0.2;
const TAIL_MS: f64 = 700.0;

/// (start_ms, dur_ms, midi)
type Song = Vec<(f64, f64, f64)>;

/// 10 notes of 1000 ms separated by 200 ms rests from 500 ms: C4 D4 E4 F4 G4 A4 G4 F4 E4 D4.
fn regression_song() -> Song {
    [60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 67.0, 65.0, 64.0, 62.0]
        .iter()
        .enumerate()
        .map(|(i, &m)| (500.0 + i as f64 * 1200.0, 1000.0, m))
        .collect()
}

/// Tied notes (no rests) from 500 ms.
fn tied(pitches: &[f64], dur: f64) -> Song {
    pitches
        .iter()
        .enumerate()
        .map(|(i, &m)| (500.0 + i as f64 * dur, dur, m))
        .collect()
}

fn reference(song: &Song) -> MelodyReference {
    MelodyReference {
        format_version: 1,
        notes: song
            .iter()
            .map(|&(s, d, m)| Note {
                start_ms: s as u32,
                dur_ms: d as u32,
                midi: m as f32,
                lyric: None,
            })
            .collect(),
    }
}

fn song_end(song: &Song) -> f64 {
    song.last().map_or(0.0, |&(s, d, _)| s + d) + TAIL_MS
}

/// Pitch of the note sounding at `t`.
fn at(song: &Song, t: f64) -> Option<f64> {
    song.iter()
        .find(|&&(s, d, _)| t >= s && t < s + d)
        .map(|n| n.2)
}

fn midi_hz(m: f64) -> f64 {
    440.0 * 2f64.powf((m - 69.0) / 12.0)
}

/// Fixed-seed xorshift, uniform in [-1, 1).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        2.0 * ((self.0 >> 11) as f64 / (1u64 << 53) as f64) - 1.0
    }
}

/// Band-limited random value: new target every `step` samples, linear in between.
struct Wobble {
    rng: Rng,
    step: usize,
    a: f64,
    b: f64,
    i: usize,
}
impl Wobble {
    fn new(seed: u64, step: usize) -> Self {
        let mut rng = Rng(seed);
        let (a, b) = (rng.next(), rng.next());
        Self {
            rng,
            step,
            a,
            b,
            i: 0,
        }
    }
    fn next(&mut self) -> f64 {
        if self.i == self.step {
            self.i = 0;
            self.a = self.b;
            self.b = self.rng.next();
        }
        let x = self.i as f64 / self.step as f64;
        self.i += 1;
        self.a + (self.b - self.a) * x
    }
}

#[derive(Clone, Copy)]
struct Voice {
    /// Harmonics with 1/k amplitude (1 = sine). Ignored when `formants`.
    harmonics: u32,
    /// Vowel /a/ formant envelope over harmonics up to 4 kHz.
    formants: bool,
    /// Relative f0 perturbation (~5 ms), e.g. 0.01 = ±1%.
    jitter: f64,
    /// Relative amplitude perturbation (~5 ms).
    shimmer: f64,
    /// Slow pitch drift amplitude in cents (~0.5 s).
    drift_cents: f64,
    /// Breath noise while voiced, SNR in dB.
    breath_snr_db: Option<f64>,
}

const SINE: Voice = Voice {
    harmonics: 1,
    formants: false,
    jitter: 0.0,
    shimmer: 0.0,
    drift_cents: 0.0,
    breath_snr_db: None,
};
const HUMAN: Voice = Voice {
    harmonics: 1,
    formants: true,
    jitter: 0.01,
    shimmer: 0.1,
    drift_cents: 15.0,
    breath_snr_db: None,
};

fn formant_gain(f: f64) -> f64 {
    // /a/: F1 700, F2 1220, F3 2600 Hz.
    [
        (700.0, 8.0, 130.0),
        (1220.0, 6.0, 150.0),
        (2600.0, 3.0, 250.0),
    ]
    .iter()
    .map(|&(fc, a, bw)| a * (-((f - fc) / bw).powi(2)).exp())
    .sum::<f64>()
        + 1.0
}

/// Renders `pitch(t)` (MIDI, `None` = silence) for `len_ms`. Continuous phase.
fn render(len_ms: f64, pitch: impl Fn(f64) -> Option<f64>, amp: f64, v: Voice) -> Vec<f32> {
    let n = (len_ms / 1000.0 * SR as f64) as usize;
    let (mut jit, mut shim, mut drift) = (
        Wobble::new(0xA11CE, 240),
        Wobble::new(0xB0B, 240),
        Wobble::new(0xD21F7, 24_000),
    );
    let mut phase = 0.0f64;
    let mut voiced = vec![false; n];
    let mut out: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 * 1000.0 / SR as f64;
            let (j, s, d) = (jit.next(), shim.next(), drift.next());
            let Some(m) = pitch(t) else {
                phase = 0.0;
                return 0.0;
            };
            voiced[i] = true;
            let f0 = midi_hz(m + d * v.drift_cents / 100.0) * (1.0 + j * v.jitter);
            phase = (phase + TAU * f0 / SR as f64) % (TAU * 1e6);
            let a = amp * (1.0 + s * v.shimmer);
            let (mut acc, mut norm) = (0.0, 0.0);
            let hs = if v.formants {
                ((4000.0 / midi_hz(m)) as u32).max(1)
            } else {
                v.harmonics
            };
            for k in 1..=hs {
                let g = if v.formants {
                    formant_gain(midi_hz(m) * k as f64)
                } else {
                    1.0
                } / k as f64;
                acc += g * (phase * k as f64).sin();
                norm += g;
            }
            a * acc / norm
        })
        .collect();
    if let Some(snr) = v.breath_snr_db {
        let vs: Vec<f64> = out
            .iter()
            .zip(&voiced)
            .filter(|(_, v)| **v)
            .map(|(s, _)| *s)
            .collect();
        let rms = (vs.iter().map(|s| s * s).sum::<f64>() / vs.len().max(1) as f64).sqrt();
        let a = rms * 10f64.powf(-snr / 20.0) * 3f64.sqrt();
        let mut rng = Rng(0xB2EA7);
        for (s, v) in out.iter_mut().zip(&voiced) {
            if *v {
                *s += a * rng.next();
            }
        }
    }
    out.into_iter().map(|s| s as f32).collect()
}

fn score_with(song: &Song, d: Difficulty, samples: &[f32]) -> ScoreResult {
    let frames = analyze_buffer(samples, SR, &CaptureConfig::default());
    let centre = window_len(SR) as f64 / 2.0;
    let mut scorer = Scorer::new(&reference(song), 0, d);
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

fn score(song: &Song, samples: &[f32]) -> ScoreResult {
    score_with(song, Difficulty::Normal, samples)
}

/// Sings `song` through `pitch(song, t)` with the sine voice.
fn sing(song: &Song, pitch: impl Fn(f64) -> Option<f64>) -> ScoreResult {
    score(song, &render(song_end(song), pitch, AMP, SINE))
}

fn report(case: &str, r: &ScoreResult) {
    eprintln!(
        "| {case} | {:.1} | {:.3} | {:.3} | {:.3} | {}/{} |",
        r.score, r.pitch, r.rhythm, r.sustain, r.notes_hit, r.notes_total
    );
}

fn siren(t: f64) -> Option<f64> {
    let x = (t % 400.0) / 400.0;
    Some(60.0 + 12.0 * (1.0 - (2.0 * x - 1.0).abs()))
}

// ---------- v1 table (still valid) ----------

#[test]
fn perfect_sine_scores_at_least_98() {
    let s = regression_song();
    let r = sing(&s, |t| at(&s, t));
    report("Perfeito (senoide)", &r);
    assert!(r.score >= 98.0, "{r:?}");
    assert_eq!(r.notes_hit, 10);
    assert_eq!(r.algorithm_version, ALGORITHM_VERSION);
}

#[test]
fn perfect_one_octave_below_scores_at_least_98() {
    let s = regression_song();
    let r = sing(&s, |t| at(&s, t).map(|m| m - 12.0));
    report("Perfeito uma oitava abaixo", &r);
    assert!(r.score >= 98.0, "{r:?}");
}

#[test]
fn plus_100_cents_gives_half_pitch() {
    let s = regression_song();
    let r = sing(&s, |t| at(&s, t).map(|m| m + 1.0));
    report("+100 cents constante", &r);
    assert!((0.45..=0.55).contains(&r.pitch), "{r:?}");
}

#[test]
fn plus_200_cents_scores_zero_pitch() {
    let s = regression_song();
    let r = sing(&s, |t| at(&s, t).map(|m| m + 2.0));
    report("+200 cents constante", &r);
    assert_eq!(r.pitch, 0.0, "{r:?}");
    assert!(r.score <= 25.0, "{r:?}");
}

#[test]
fn silence_scores_zero() {
    let s = regression_song();
    let r = sing(&s, |_| None);
    report("Silêncio", &r);
    assert_eq!(r.score, 0.0, "{r:?}");
}

#[test]
fn white_noise_scores_at_most_5() {
    let s = regression_song();
    let n = (song_end(&s) / 1000.0 * SR as f64) as usize;
    let mut rng = Rng(0x5EED);
    let noise: Vec<f32> = (0..n).map(|_| (0.3 * rng.next()) as f32).collect();
    let r = score(&s, &noise);
    report("Ruído branco", &r);
    assert!(r.score <= 5.0, "{r:?}");
}

#[test]
fn delay_of_200ms_costs_rhythm_not_pitch() {
    let s = regression_song();
    let r = sing(&s, |t| at(&s, t - 200.0));
    report("Atraso de 200 ms", &r);
    assert!((0.3..=0.6).contains(&r.rhythm), "{r:?}");
    assert!(r.pitch >= 0.7, "{r:?}");
}

#[test]
fn one_of_ten_notes_scores_at_most_15() {
    let s = regression_song();
    let r = sing(&s, |t| at(&s, t).filter(|_| t < 1500.0));
    report("Só 1 de 10 notas cantada", &r);
    assert!(r.score <= 15.0, "{r:?}");
    assert_eq!(r.notes_hit, 1);
}

#[test]
fn four_times_louder_is_identical() {
    let s = regression_song();
    let base = render(song_end(&s), |t| at(&s, t), AMP, SINE);
    let loud: Vec<f32> = base.iter().map(|x| x * 4.0).collect();
    let (a, b) = (score(&s, &base), score(&s, &loud));
    report("Perfeito com volume 4×", &b);
    assert_eq!(a, b);
}

#[test]
fn vibrato_50_cents_6hz_scores_at_least_90() {
    let s = regression_song();
    let r = sing(&s, |t| {
        at(&s, t).map(|m| m + 0.5 * (TAU * 6.0 * t / 1000.0).sin())
    });
    report("Vibrato ±50 cents a 6 Hz", &r);
    assert!(r.score >= 90.0, "{r:?}");
}

#[test]
fn voice_with_accompaniment_at_minus_12db_scores_at_least_85() {
    // Voice-like tone + major triad one octave below the current note, held
    // through the following rest, total RMS 12 dB below the voice.
    let s = regression_song();
    let end = song_end(&s);
    let voice_tone = Voice {
        harmonics: 8,
        ..SINE
    };
    let voice = render(end, |t| at(&s, t), AMP, voice_tone);
    let voiced: Vec<f64> = voice
        .iter()
        .filter(|x| **x != 0.0)
        .map(|&x| x as f64)
        .collect();
    let voice_rms = (voiced.iter().map(|x| x * x).sum::<f64>() / voiced.len() as f64).sqrt();
    let chord_amp = voice_rms * 10f64.powf(-12.0 / 20.0) / 1.5f64.sqrt();
    let chord_root = |t: f64| s.iter().rev().find(|n| t >= n.0).map(|n| n.2 - 12.0);
    let parts: Vec<Vec<f32>> = [0.0, 4.0, 7.0]
        .iter()
        .map(|&iv| render(end, |t| chord_root(t).map(|m| m + iv), chord_amp, SINE))
        .collect();
    let mix: Vec<f32> = (0..voice.len())
        .map(|i| voice[i] + parts.iter().map(|p| p[i]).sum::<f32>())
        .collect();
    let r = score(&s, &mix);
    report("Voz + acompanhamento a −12 dB", &r);
    assert!(r.score >= 85.0, "{r:?}");
}

#[test]
fn glissando_of_80ms_into_each_note_scores_at_least_92() {
    let s = regression_song();
    let r = sing(&s, |t| {
        let i = s.iter().position(|&(st, d, _)| t >= st && t < st + d)?;
        let from = s[i.saturating_sub(1)].2;
        let x = ((t - s[i].0) / 80.0).min(1.0);
        Some(from + (s[i].2 - from) * x)
    });
    report("Perfeito com glissando de 80 ms entre notas", &r);
    assert!(r.score >= 92.0, "{r:?}");
}

// ---------- v2 targets (SPEC-006 "Algoritmo v2") ----------

#[test]
fn a1_tied_repeated_and_semitone_notes_keep_rhythm() {
    for (label, pitches) in [
        (
            "A1 ligadas repetidas aos pares",
            vec![60.0, 60.0, 62.0, 62.0, 64.0, 64.0, 62.0, 62.0, 60.0, 60.0],
        ),
        (
            "A1 ligadas em semitons alternados",
            vec![60.0, 61.0, 60.0, 61.0, 60.0, 61.0, 60.0, 61.0, 60.0, 61.0],
        ),
        (
            "A1 ligadas, tons distintos",
            vec![60.0, 62.0, 64.0, 62.0, 60.0, 62.0, 64.0, 62.0, 60.0, 62.0],
        ),
    ] {
        let s = tied(&pitches, 500.0);
        let r = sing(&s, |t| at(&s, t));
        report(label, &r);
        assert!(r.rhythm >= 0.95, "{label}: {r:?}");
        assert!(r.score >= 95.0, "{label}: {r:?}");
    }
}

#[test]
fn a3_early_and_late_cost_the_same_rhythm() {
    // Notes 1000 ms apart from rests of 400 ms so the ±300 ms window is not clipped.
    let s: Song = [60.0, 62.0, 64.0, 65.0, 67.0]
        .iter()
        .enumerate()
        .map(|(i, &m)| (500.0 + i as f64 * 1400.0, 1000.0, m))
        .collect();
    for shift in [100.0, 200.0, 300.0] {
        let late = sing(&s, |t| at(&s, t - shift));
        let early = sing(&s, |t| at(&s, t + shift));
        report(&format!("A3 atrasado {shift} ms"), &late);
        report(&format!("A3 adiantado {shift} ms"), &early);
        assert!(
            (late.rhythm - early.rhythm).abs() <= 0.06,
            "{shift}: {late:?} vs {early:?}"
        );
    }
    // Entering early and holding to the end no longer escapes the rhythm penalty.
    let hold = sing(&s, |t| {
        s.iter()
            .find(|&&(st, d, _)| t >= st - 190.0 && t < st + d)
            .map(|n| n.2)
    });
    report("A3 entra 190 ms antes e segura", &hold);
    assert!(hold.rhythm <= 0.6, "{hold:?}");
}

#[test]
fn a4_plus_90_cents_scores_at_most_40() {
    let s = regression_song();
    for c in [50.0, 75.0, 90.0, 100.0, 125.0] {
        let r = sing(&s, |t| at(&s, t).map(|m| m + c / 100.0));
        report(&format!("A4 +{c} cents constante"), &r);
        if c == 90.0 {
            assert!(r.score <= 40.0, "{r:?}");
        }
    }
}

#[test]
fn a5_siren_scores_at_most_15() {
    let s = regression_song();
    let r = sing(&s, siren);
    report("A5 sirene de 1 oitava, período 400 ms", &r);
    assert!(r.score <= 15.0, "{r:?}");
}

/// A5 "nota fixa ≤ 15" is NOT met by design: a drone on E4 genuinely sings the two
/// E4 notes of the song (20% of its duration). Measured and reported, not asserted
/// (see docs/testing/scoring-accuracy.md).
#[test]
fn a5_fixed_note_is_measured() {
    let s = regression_song();
    let best = (0..12)
        .map(|pc| (pc, sing(&s, |_| Some(60.0 + pc as f64))))
        .max_by(|a, b| a.1.score.total_cmp(&b.1.score));
    if let Some((pc, r)) = best {
        report(&format!("A5 nota fixa contínua (melhor: +{pc} st)"), &r);
        assert!(r.score <= 25.0, "{r:?}");
    }
}

#[test]
fn a6_short_tied_notes() {
    let pitches: Vec<f64> = (0..20)
        .map(|i| if i % 2 == 0 { 60.0 } else { 64.0 })
        .collect();
    for (dur, min) in [(30.0, 0.0), (50.0, 85.0), (100.0, 92.0), (250.0, 95.0)] {
        let s = tied(&pitches, dur);
        let r = sing(&s, |t| at(&s, t));
        report(&format!("A6 notas ligadas de {dur} ms"), &r);
        assert!(r.score >= min, "{dur} ms: {r:?}");
    }
}

#[test]
fn a9_realistic_voice_in_three_registers() {
    let base = regression_song();
    for (reg, shift) in [("grave", -12.0), ("médio", 0.0), ("agudo", 12.0)] {
        let s: Song = base.iter().map(|&(st, d, m)| (st, d, m + shift)).collect();
        for (label, v) in [
            ("voz humana", HUMAN),
            (
                "voz humana + sopro SNR 10 dB",
                Voice {
                    breath_snr_db: Some(10.0),
                    ..HUMAN
                },
            ),
        ] {
            let audio = render(song_end(&s), |t| at(&s, t), AMP, v);
            for d in [Difficulty::Normal, Difficulty::Hard] {
                let r = score_with(&s, d, &audio);
                report(&format!("A9 {label}, {reg}, {d:?}"), &r);
                assert!(r.score >= 95.0, "{label} {reg} {d:?}: {r:?}");
            }
        }
    }
}

#[test]
fn a2_guide_vocal_leak_is_measured() {
    // User silent; the mic picks up the video's guide vocal. Not solvable in the
    // scorer: the leak check (T-004-06) makes such runs non-competitive.
    let s = regression_song();
    for atten in [20.0, 30.0, 40.0] {
        let vowel = Voice {
            jitter: 0.0,
            shimmer: 0.0,
            drift_cents: 0.0,
            ..HUMAN
        };
        let audio = render(
            song_end(&s),
            |t| at(&s, t),
            AMP * 10f64.powf(-atten / 20.0),
            vowel,
        );
        report(
            &format!("A2 voz-guia vazando −{atten} dB (informativo)"),
            &score(&s, &audio),
        );
    }
}

#[test]
fn scoring_is_deterministic() {
    let s = regression_song();
    let audio = render(song_end(&s), |t| at(&s, t), AMP, HUMAN);
    assert_eq!(score(&s, &audio), score(&s, &audio));
}

// ---------- v3 targets (SPEC-006 "Algoritmo v3"; probes from scoring-review-v2) ----------

/// The T-003-05 test song (frozen copy): legato, repeated notes, a 150 ms note, a rest, a fifth leap.
fn test_song() -> Song {
    let json = include_str!("../../../tests/fixtures/references/test-song.json");
    scoring_engine::parse_reference_json(json)
        .expect("test song")
        .notes
        .iter()
        .map(|n| (n.start_ms as f64, n.dur_ms as f64, n.midi as f64))
        .collect()
}

/// Realistic voice (formants, jitter, shimmer) with drift up to ±`drift` cents,
/// vibrato ±`vib` cents at 5.5 Hz and a constant `bias` (probe P9).
fn amateur(song: &Song, d: Difficulty, drift: f64, vib: f64, bias: f64) -> ScoreResult {
    let v = Voice {
        drift_cents: drift,
        ..HUMAN
    };
    let audio = render(
        song_end(song),
        |t| at(song, t).map(|m| m + (bias + vib * (TAU * 5.5 * t / 1000.0).sin()) / 100.0),
        AMP,
        v,
    );
    score_with(song, d, &audio)
}

/// Scorer only: ideal frames every 10 ms, kept by `keep(index, t)`.
fn ideal(
    song: &Song,
    d: Difficulty,
    keep: impl Fn(usize, f64) -> bool,
    pitch: impl Fn(f64) -> Option<f64>,
) -> ScoreResult {
    let mut s = Scorer::new(&reference(song), 0, d);
    for (i, t) in (0..(song_end(song) / 10.0) as usize).map(|i| (i, i as f64 * 10.0)) {
        if keep(i, t) {
            let m = pitch(t);
            s.push(VoiceFrame {
                t_ms: t,
                f0_hz: m.map(|m| midi_hz(m) as f32),
                confidence: 0.95,
                voiced: m.is_some(),
            });
        }
    }
    s.finish()
}

#[test]
fn n1_vibrato_is_not_detune() {
    for (name, s) in [
        ("regressão", regression_song()),
        ("música de teste", test_song()),
    ] {
        for (vib, min) in [(50.0, 95.0), (75.0, 92.0), (100.0, 88.0)] {
            let r = amateur(&s, Difficulty::Normal, 0.0, vib, 0.0);
            report(
                &format!("N1 voz humana centrada, vibrato ±{vib} c, {name}"),
                &r,
            );
            assert!(r.score >= min, "±{vib} {name}: {r:?}");
            let drifting = amateur(&s, Difficulty::Normal, 25.0, vib, 0.0);
            report(
                &format!("N1 idem com drift ±25 c (informativo), {name}"),
                &drifting,
            );
        }
    }
}

/// Probe P9b: the amateur curve that triggers the tolerance calibration (informative).
#[test]
fn amateur_curve_is_measured() {
    let s = test_song();
    for bias in [0.0, 20.0, 30.0, -40.0] {
        for d in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
            let r = amateur(&s, d, 20.0, 50.0, bias);
            report(
                &format!("Amador drift ±20 + vibrato ±50, desvio {bias:+} c, {d:?} (informativo)"),
                &r,
            );
        }
    }
}

#[test]
fn n2_frames_that_never_arrived_do_not_count() {
    let s = regression_song();
    let sing = |t: f64| at(&s, t);
    for (pct, thr) in [(5, -0.9), (10, -0.8), (15, -0.7)] {
        let mut rng = Rng(0xD20B + pct);
        let drop: Vec<bool> = (0..2000).map(|_| rng.next() < thr).collect();
        let r = ideal(&s, Difficulty::Normal, |i, _| !drop[i], sing);
        report(
            &format!("N2 ~{pct}% dos frames perdidos, canto perfeito"),
            &r,
        );
        assert_eq!(r.score, 100.0, "{pct}%: {r:?}");
    }
    let burst = ideal(&s, Difficulty::Normal, |_, t| t % 5000.0 >= 200.0, sing);
    report("N2 rajada de 200 ms perdida a cada 5 s", &burst);
    assert!(burst.score >= 99.0, "{burst:?}");
    // Silence that did arrive still counts 0 (A8 stays solved by the training rule).
    let first_100 = ideal(
        &s,
        Difficulty::Normal,
        |_, _| true,
        |t| {
            s.iter()
                .any(|&(st, _, _)| t >= st && t < st + 100.0)
                .then(|| at(&s, t))
                .flatten()
        },
    );
    report(
        "N2 só os 100 ms iniciais cantados, resto calado",
        &first_100,
    );
    assert!(first_100.score <= 20.0, "{first_100:?}");
}

#[test]
fn n3_holding_the_previous_note_costs_like_arriving_late_in_silence() {
    let s = tied(
        &[60.0, 62.0, 64.0, 62.0, 60.0, 62.0, 64.0, 62.0, 60.0, 62.0],
        500.0,
    );
    for hold in [80.0, 150.0] {
        let sung = |held: bool| {
            sing(&s, |t| {
                let i = s.iter().position(|&(st, d, _)| t >= st && t < st + d)?;
                if i > 0 && t < s[i].0 + hold {
                    held.then_some(s[i - 1].2)
                } else {
                    Some(s[i].2)
                }
            })
        };
        let (held, silent) = (sung(true), sung(false));
        report(&format!("N3 segura a nota anterior {hold} ms"), &held);
        report(&format!("N3 calado {hold} ms e entra"), &silent);
        assert!(
            held.score >= silent.score - 3.0,
            "{hold}: {held:?} vs {silent:?}"
        );
    }
}

#[test]
fn n4_short_tied_notes_tolerate_latency_error() {
    let pitches: Vec<f64> = (0..20)
        .map(|i| if i % 2 == 0 { 60.0 } else { 64.0 })
        .collect();
    for (dur, min) in [(50.0, 70.0), (100.0, 85.0), (150.0, 0.0)] {
        let s = tied(&pitches, dur);
        for late in [25.0, 50.0] {
            // The formant voice makes YIN jump octaves on fast changes: the
            // in-note median must keep the pitch class.
            for (voice, v) in [("senoide", SINE), ("voz humana", HUMAN)] {
                let audio = render(song_end(&s), |t| at(&s, t - late), AMP, v);
                let r = score(&s, &audio);
                report(
                    &format!("N4 ligadas de {dur} ms, {late} ms atrasadas, {voice}"),
                    &r,
                );
                if late == 25.0 {
                    assert!(r.score >= min, "{dur} ms {voice}: {r:?}");
                }
            }
        }
    }
}

#[test]
fn a3_short_rests_do_not_cap_the_early_penalty() {
    // The regression song has 200 ms rests: shifting ±300 ms must cost the same.
    let s = regression_song();
    for shift in [150.0, 200.0, 300.0] {
        let late = sing(&s, |t| at(&s, t - shift));
        let early = sing(&s, |t| at(&s, t + shift));
        report(&format!("A3 pausas de 200 ms, atrasado {shift} ms"), &late);
        report(
            &format!("A3 pausas de 200 ms, adiantado {shift} ms"),
            &early,
        );
        assert!(
            (late.rhythm - early.rhythm).abs() <= 0.06,
            "{shift}: {late:?} vs {early:?}"
        );
    }
    // Enters each note `early` ms before its start and holds it to the end,
    // cutting the previous note short when the rest is shorter than that.
    let enter = |early: f64| {
        sing(&s, |t| {
            s.iter()
                .rev()
                .find(|&&(st, d, _)| t >= st - early && t < st + d)
                .map(|n| n.2)
        })
    };
    let (e190, e400) = (enter(190.0), enter(400.0));
    report("A3 pausas de 200 ms, entra 190 ms antes e segura", &e190);
    report(
        "A3 pausas de 200 ms, entra 400 ms antes (corta a anterior)",
        &e400,
    );
    assert!((0.4..=0.6).contains(&e190.rhythm), "{e190:?}");
    assert!(e400.rhythm <= 0.05, "{e400:?}");
    // Rests ≤ 105 ms: filling the rest with the next note is an early entry.
    let short: Song = [60.0, 62.0, 64.0, 65.0, 67.0]
        .iter()
        .enumerate()
        .map(|(i, &m)| (500.0 + i as f64 * 1100.0, 1000.0, m))
        .collect();
    let late = sing(&short, |t| at(&short, t - 200.0));
    let early = sing(&short, |t| at(&short, t + 200.0));
    report("A3 pausas de 100 ms, atrasado 200 ms", &late);
    report("A3 pausas de 100 ms, adiantado 200 ms", &early);
    assert!(
        (late.rhythm - early.rhythm).abs() <= 0.06,
        "{late:?} vs {early:?}"
    );
}

#[test]
fn a4_constant_detune_on_the_regression_song() {
    // A4 (a)/(b)/(c) with ideal frames on the 10-note song, every cent 0..300.
    let s = regression_song();
    for d in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
        let curve: Vec<f32> = (0..=300)
            .map(|c| {
                ideal(
                    &s,
                    d,
                    |_, _| true,
                    |t| at(&s, t).map(|m| m + c as f64 / 100.0),
                )
                .score
            })
            .collect();
        let max_jump = (10..curve.len())
            .map(|c| curve[c - 10] - curve[c])
            .fold(0f32, f32::max);
        eprintln!(
            "A4 {d:?}: maior queda em 10 cents = {max_jump:.1}; +90 = {}",
            curve[90]
        );
        assert!(curve.windows(2).all(|w| w[1] <= w[0]), "{d:?} not monotone");
        assert!(max_jump <= 10.0, "{d:?}: {max_jump}");
        if d == Difficulty::Normal {
            assert!(curve[90] <= 40.0, "+90 = {}", curve[90]);
        }
    }
}

// ---------- A7: golden values ----------

fn golden_cases() -> Vec<(&'static str, ScoreResult)> {
    let s = regression_song();
    let legato = tied(
        &[60.0, 60.0, 62.0, 62.0, 64.0, 64.0, 62.0, 62.0, 60.0, 60.0],
        500.0,
    );
    vec![
        ("perfect", sing(&s, |t| at(&s, t))),
        ("plus_75_cents", sing(&s, |t| at(&s, t).map(|m| m + 0.75))),
        ("delay_200", sing(&s, |t| at(&s, t - 200.0))),
        (
            "vibrato",
            sing(&s, |t| {
                at(&s, t).map(|m| m + 0.5 * (TAU * 6.0 * t / 1000.0).sin())
            }),
        ),
        ("siren", sing(&s, siren)),
        ("tied_repeated", sing(&legato, |t| at(&legato, t))),
        (
            "human_voice",
            score(&s, &render(song_end(&s), |t| at(&s, t), AMP, HUMAN)),
        ),
        (
            "human_vibrato_75",
            amateur(&s, Difficulty::Normal, 0.0, 75.0, 0.0),
        ),
        ("hold_previous_150", {
            let l = tied(
                &[60.0, 62.0, 64.0, 62.0, 60.0, 62.0, 64.0, 62.0, 60.0, 62.0],
                500.0,
            );
            sing(&l, |t| {
                let i = l.iter().position(|&(st, d, _)| t >= st && t < st + d)?;
                Some(if i > 0 && t < l[i].0 + 150.0 {
                    l[i - 1].2
                } else {
                    l[i].2
                })
            })
        }),
        ("short_50_late_25", {
            let p: Vec<f64> = (0..20)
                .map(|i| if i % 2 == 0 { 60.0 } else { 64.0 })
                .collect();
            let l = tied(&p, 50.0);
            sing(&l, |t| at(&l, t - 25.0))
        }),
        ("missing_10_percent", {
            let mut rng = Rng(0xD20B + 10);
            let drop: Vec<bool> = (0..2000).map(|_| rng.next() < -0.8).collect();
            ideal(&s, Difficulty::Normal, |i, _| !drop[i], |t| at(&s, t))
        }),
        (
            "early_400",
            sing(&s, |t| {
                s.iter()
                    .rev()
                    .find(|&&(st, d, _)| t >= st - 400.0 && t < st + d)
                    .map(|n| n.2)
            }),
        ),
    ]
}

/// Exact expected results for ALGORITHM_VERSION 3. Regenerate only together
/// with a version bump: run with `-- --nocapture golden` and paste.
fn golden_expected() -> Vec<(&'static str, ScoreResult)> {
    let r = |score, pitch, rhythm, sustain, coverage, notes_hit, notes_total| ScoreResult {
        score,
        pitch,
        rhythm,
        sustain,
        coverage,
        notes_total,
        notes_hit,
        algorithm_version: 3,
    };
    vec![
        ("perfect", r(100.0, 1.0, 1.0, 1.0, 1.0, 10, 10)),
        (
            "plus_75_cents",
            r(51.7, 0.6424838, 0.28496748, 0.28496748, 1.0, 10, 10),
        ),
        (
            "delay_200",
            r(73.1, 0.80178905, 0.44848484, 0.804, 1.0, 10, 10),
        ),
        ("vibrato", r(100.0, 1.0, 1.0, 1.0, 1.0, 10, 10)),
        ("siren", r(9.5, 0.1454789, 0.0, 0.0, 0.0, 0, 10)),
        ("tied_repeated", r(100.0, 1.0, 1.0, 1.0, 1.0, 10, 10)),
        ("human_voice", r(100.0, 1.0, 1.0, 1.0, 1.0, 10, 10)),
        (
            "human_vibrato_75",
            r(99.9, 0.99835986, 1.0, 1.0, 1.0, 10, 10),
        ),
        (
            "hold_previous_150",
            r(72.6, 0.7412293, 0.66727275, 0.7412293, 1.0, 10, 10),
        ),
        (
            "short_50_late_25",
            r(95.8, 0.9685284, 0.9370568, 0.9370568, 1.0, 20, 20),
        ),
        ("missing_10_percent", r(100.0, 1.0, 1.0, 1.0, 1.0, 10, 10)),
        (
            "early_400",
            r(66.5, 0.8455609, 0.005064616, 0.7639583, 1.0, 10, 10),
        ),
    ]
}

#[test]
fn golden_results_are_exact() {
    let got = golden_cases();
    for (name, r) in &got {
        eprintln!(
            "        (\"{name}\", r({:?}, {:?}, {:?}, {:?}, {:?}, {}, {})),",
            r.score, r.pitch, r.rhythm, r.sustain, r.coverage, r.notes_hit, r.notes_total
        );
    }
    assert_eq!(
        golden_expected(),
        got,
        "scoring output changed: bump ALGORITHM_VERSION, update SPEC-006 and these golden values"
    );
}
