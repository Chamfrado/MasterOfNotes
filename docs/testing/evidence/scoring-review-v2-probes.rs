// ARCHIVED COPY (not compiled). To reproduce: copy to crates/scoring-engine/tests/qa_probe.rs and create
// crates/scoring-engine/tests/v1_scorer.inc from `git show 86c604b:crates/scoring-engine/src/scorer.rs`
// (drop its #[cfg(test)] module; replace `use crate::MelodyReference` with `use scoring_engine::MelodyReference`).

//! QA probes (Agent 4): v1 × v2 on the same audio. MEASUREMENT ONLY, local copy, NOT committed in the
//! crate (owner: Agent 2). v1 = scorer.rs @ 86c604b included as a module (tests/v1_scorer.inc).
//! Run: cargo test -p scoring-engine --test qa_probe -- --nocapture --test-threads=1
//! Findings: docs/testing/evidence/scoring-review.md (v1), scoring-review-v2.md (v2)

#[allow(dead_code)]
mod v1 {
    include!("v1_scorer.inc");
}

use audio_engine::{AnalysisFrame, CaptureConfig, analyze_buffer, window_len};
use scoring_engine::{Difficulty, MelodyReference, Note, Scorer, VoiceFrame};
use std::f64::consts::TAU;

const SR: u32 = 48_000;
const AMP: f64 = 0.2;

/// (start_ms, dur_ms, midi)
type Song = Vec<(f64, f64, f64)>;

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

/// Same fixture as tests/regression.rs: 10 × 1000 ms notes, 200 ms rests, from 500 ms.
fn regression_song() -> Song {
    let m = [60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 67.0, 65.0, 64.0, 62.0];
    m.iter()
        .enumerate()
        .map(|(i, &p)| (500.0 + i as f64 * 1200.0, 1000.0, p))
        .collect()
}

/// Back-to-back notes (no rests) from 500 ms.
fn legato(pitches: &[f64], dur: f64) -> Song {
    pitches
        .iter()
        .enumerate()
        .map(|(i, &p)| (500.0 + i as f64 * dur, dur, p))
        .collect()
}

fn song_end(song: &Song) -> f64 {
    song.last().map_or(0.0, |&(s, d, _)| s + d) + 700.0
}

fn note_at(song: &Song, t: f64) -> Option<(f64, f64, f64)> {
    song.iter().copied().find(|&(s, d, _)| t >= s && t < s + d)
}

fn midi_hz(m: f64) -> f64 {
    440.0 * 2f64.powf((m - 69.0) / 12.0)
}

struct Rng(u64);
impl Rng {
    /// Uniform in [-1, 1).
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        2.0 * ((self.0 >> 11) as f64 / (1u64 << 53) as f64) - 1.0
    }
}

/// Band-limited random value: new target every `step` samples, linear interpolation.
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
    /// Vowel /a/ formant envelope over harmonics up to 4 kHz; false = pure sine.
    formants: bool,
    /// Relative f0 perturbation (fast, ~5 ms), e.g. 0.01 = ±1 %.
    jitter: f64,
    /// Relative amplitude perturbation (~5 ms).
    shimmer: f64,
    /// Slow pitch drift amplitude in cents (~0.5 s).
    drift_cents: f64,
    /// Aspiration noise while voiced, SNR in dB relative to the voice RMS.
    breath_snr_db: Option<f64>,
}

const SINE: Voice = Voice {
    formants: false,
    jitter: 0.0,
    shimmer: 0.0,
    drift_cents: 0.0,
    breath_snr_db: None,
};
const VOWEL: Voice = Voice {
    formants: true,
    ..SINE
};
const HUMAN: Voice = Voice {
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

/// Pitch (MIDI) at time t (ms), None = silence. Continuous phase. Deterministic.
fn render(len_ms: f64, pitch: impl Fn(f64) -> Option<f64>, amp: f64, v: Voice) -> Vec<f32> {
    let n = (len_ms / 1000.0 * SR as f64) as usize;
    let mut jit = Wobble::new(0xA11CE, 240);
    let mut shim = Wobble::new(0xB0B, 240);
    let mut drift = Wobble::new(0xD21F7, 24_000);
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
            if !v.formants {
                return a * phase.sin();
            }
            let base = midi_hz(m);
            let hs = ((4000.0 / base) as u32).max(1);
            let (mut acc, mut norm) = (0.0, 0.0);
            for k in 1..=hs {
                let g = formant_gain(base * k as f64) / k as f64;
                acc += g * (phase * k as f64).sin();
                norm += g;
            }
            a * 1.5 * acc / norm
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

fn frames_to_voice(frames: &[AnalysisFrame]) -> Vec<VoiceFrame> {
    let centre = window_len(SR) as f64 / 2.0;
    frames
        .iter()
        .map(|f| VoiceFrame {
            t_ms: (f.sample_index as f64 + centre) * 1000.0 / SR as f64,
            f0_hz: f.f0_hz,
            confidence: f.confidence,
            voiced: f.voiced,
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct R {
    score: f32,
    p: f32,
    r: f32,
    s: f32,
    hit: u32,
    total: u32,
}
#[derive(Clone, Copy, Debug)]
struct Both {
    v1: R,
    v2: R,
}

fn score_frames(song: &Song, d: Difficulty, frames: &[VoiceFrame]) -> Both {
    let d1 = match d {
        Difficulty::Easy => v1::Difficulty::Easy,
        Difficulty::Normal => v1::Difficulty::Normal,
        Difficulty::Hard => v1::Difficulty::Hard,
    };
    let mut a = v1::Scorer::new(&reference(song), 0, d1);
    let mut b = Scorer::new(&reference(song), 0, d);
    for f in frames {
        a.push(v1::VoiceFrame {
            t_ms: f.t_ms,
            f0_hz: f.f0_hz,
            confidence: f.confidence,
            voiced: f.voiced,
        });
        b.push(*f);
    }
    let (x, y) = (a.finish(), b.finish());
    Both {
        v1: R { score: x.score, p: x.pitch, r: x.rhythm, s: x.sustain, hit: x.notes_hit, total: x.notes_total },
        v2: R { score: y.score, p: y.pitch, r: y.rhythm, s: y.sustain, hit: y.notes_hit, total: y.notes_total },
    }
}

/// Full pipeline: audio → analyze_buffer → Scorer (v1 and v2).
fn score_audio(song: &Song, d: Difficulty, audio: &[f32]) -> Both {
    score_frames(song, d, &frames_to_voice(&analyze_buffer(audio, SR, &CaptureConfig::default())))
}

/// Scorer only: ideal frames every 10 ms from a pitch function (MIDI).
fn score_direct(song: &Song, d: Difficulty, pitch: impl Fn(f64) -> Option<f64>) -> Both {
    let frames: Vec<VoiceFrame> = (0..(song_end(song) / 10.0) as usize)
        .map(|i| {
            let t = i as f64 * 10.0;
            let m = pitch(t);
            VoiceFrame {
                t_ms: t,
                f0_hz: m.map(|m| midi_hz(m) as f32),
                confidence: 0.95,
                voiced: m.is_some(),
            }
        })
        .collect();
    score_frames(song, d, &frames)
}

/// In-note frames: (% voiced, % voiced with octave-folded error ≤ 50 cents).
fn frame_quality(song: &Song, audio: &[f32]) -> (f64, f64) {
    let vf = frames_to_voice(&analyze_buffer(audio, SR, &CaptureConfig::default()));
    let (mut n, mut voiced, mut ok) = (0, 0, 0);
    for f in &vf {
        let Some((_, _, m)) = note_at(song, f.t_ms) else {
            continue;
        };
        n += 1;
        if let (true, Some(hz)) = (f.voiced, f.f0_hz) {
            voiced += 1;
            let e = 1200.0 * (hz as f64 / midi_hz(m)).log2();
            let e = e - 1200.0 * (e / 1200.0).round();
            ok += usize::from(e.abs() <= 50.0);
        }
    }
    (
        100.0 * voiced as f64 / n as f64,
        100.0 * ok as f64 / n as f64,
    )
}

fn cell(r: &R) -> String {
    format!("{:.1} ({:.2}/{:.2}/{:.2}) {}/{}", r.score, r.p, r.r, r.s, r.hit, r.total)
}

fn row(label: &str, b: &Both) {
    eprintln!("| {label} | {} | {} | {:+.1} |", cell(&b.v1), cell(&b.v2), b.v2.score - b.v1.score);
}

fn header(title: &str) {
    eprintln!("
### {title}
| Caso | v1 score (P/R/S) hits | v2 score (P/R/S) hits | Δ |
|---|---|---|---|");
}

#[test]
fn p1_cents_cliff() {
    let song = regression_song();
    header("P1 desvio constante (pipeline, senoide, Normal)");
    for c in [
        0.0, 50.0, 75.0, 90.0, 95.0, 99.0, 100.0, 101.0, 105.0, 125.0, 150.0,
    ] {
        let a = render(
            song_end(&song),
            |t| note_at(&song, t).map(|n| n.2 + c / 100.0),
            AMP,
            SINE,
        );
        row(
            &format!("+{c} cents"),
            &score_audio(&song, Difficulty::Normal, &a),
        );
    }
}

#[test]
fn p2_timing() {
    let song = regression_song();
    header("P2 deslocamento de tempo (pipeline, senoide; negativo = adiantado)");
    for delay in [
        -300.0, -200.0, -150.0, -100.0, -50.0, 0.0, 50.0, 100.0, 150.0, 200.0, 300.0,
    ] {
        let a = render(
            song_end(&song),
            |t| note_at(&song, t - delay).map(|n| n.2),
            AMP,
            SINE,
        );
        row(
            &format!("{delay:+} ms"),
            &score_audio(&song, Difficulty::Normal, &a),
        );
    }
    // Enter early and hold to the exact end: never loses pitch frames.
    for early in [100.0, 190.0] {
        let a = render(
            song_end(&song),
            |t| {
                song.iter()
                    .find(|&&(s, d, _)| t >= s - early && t < s + d)
                    .map(|n| n.2)
            },
            AMP,
            SINE,
        );
        row(
            &format!("entra {early} ms antes e segura até o fim"),
            &score_audio(&song, Difficulty::Normal, &a),
        );
    }
    let a = render(
        song_end(&song),
        |t| {
            song.iter()
                .find(|&&(s, d, _)| t >= s + 150.0 && t < s + d)
                .map(|n| n.2)
        },
        AMP,
        SINE,
    );
    row(
        "entra 150 ms atrasado, termina no fim",
        &score_audio(&song, Difficulty::Normal, &a),
    );
}

#[test]
fn p3_repeated_and_legato_notes() {
    header("P3 notas ligadas (pipeline, senoide, 500 ms cada, cantor perfeito)");
    let cases: [(&str, Vec<f64>); 3] = [
        (
            "tons distintos (2 st)",
            vec![60.0, 62.0, 64.0, 62.0, 60.0, 62.0, 64.0, 62.0, 60.0, 62.0],
        ),
        (
            "notas repetidas aos pares",
            vec![60.0, 60.0, 62.0, 62.0, 64.0, 64.0, 62.0, 62.0, 60.0, 60.0],
        ),
        (
            "semitons alternados",
            vec![60.0, 61.0, 60.0, 61.0, 60.0, 61.0, 60.0, 61.0, 60.0, 61.0],
        ),
    ];
    for (label, pitches) in cases {
        let song = legato(&pitches, 500.0);
        let end = song_end(&song);
        let cont = render(end, |t| note_at(&song, t).map(|n| n.2), AMP, SINE);
        row(
            &format!("{label}, contínuo"),
            &score_audio(&song, Difficulty::Normal, &cont),
        );
        // Consonant: 60 ms unvoiced gap right before each note start.
        let cons = render(
            end,
            |t| {
                let n = note_at(&song, t)?;
                let next_start = n.0 + n.1;
                (t < next_start - 60.0 || next_start >= end - 700.0).then_some(n.2)
            },
            AMP,
            SINE,
        );
        row(
            &format!("{label}, consoante 60 ms antes de cada nota"),
            &score_audio(&song, Difficulty::Normal, &cons),
        );
    }
}

#[test]
fn p4_short_notes() {
    header("P4 notas curtas (pipeline, senoide, 20 notas alternando C4/E4)");
    let pitches: Vec<f64> = (0..20)
        .map(|i| if i % 2 == 0 { 60.0 } else { 64.0 })
        .collect();
    for dur in [30.0, 50.0, 80.0, 100.0, 150.0, 250.0] {
        let song = legato(&pitches, dur);
        let a = render(
            song_end(&song),
            |t| note_at(&song, t).map(|n| n.2),
            AMP,
            SINE,
        );
        row(
            &format!("{dur} ms ligadas"),
            &score_audio(&song, Difficulty::Normal, &a),
        );
        // Same notes separated by rests of the same length.
        let spaced: Song = pitches
            .iter()
            .enumerate()
            .map(|(i, &p)| (500.0 + i as f64 * 2.0 * dur, dur, p))
            .collect();
        let a = render(
            song_end(&spaced),
            |t| note_at(&spaced, t).map(|n| n.2),
            AMP,
            SINE,
        );
        row(
            &format!("{dur} ms com pausa igual"),
            &score_audio(&spaced, Difficulty::Normal, &a),
        );
    }
}

#[test]
fn p5_realistic_voice() {
    eprintln!("\n### P5 voz realista (pipeline, cantor afinado; melodia da regressão transposta)");
    eprintln!("| Voz | Registro | Dif. | v1 | v2 | % frames vozeados | % frames ≤ 50 cents |");
    eprintln!("|---|---|---|---|---|---|---|");
    let base = regression_song();
    let voices = [
        ("senoide", SINE),
        ("vogal /a/ (formantes)", VOWEL),
        ("/a/ + jitter 1% + shimmer 10% + drift 15 c", HUMAN),
        (
            "idem + sopro SNR 20 dB",
            Voice {
                breath_snr_db: Some(20.0),
                ..HUMAN
            },
        ),
        (
            "idem + sopro SNR 10 dB",
            Voice {
                breath_snr_db: Some(10.0),
                ..HUMAN
            },
        ),
        (
            "idem + sopro SNR 5 dB",
            Voice {
                breath_snr_db: Some(5.0),
                ..HUMAN
            },
        ),
    ];
    for (reg, shift) in [
        ("grave (C3–A3)", -12.0),
        ("médio (C4–A4)", 0.0),
        ("agudo (C5–A5)", 12.0),
    ] {
        let song: Song = base.iter().map(|&(s, d, m)| (s, d, m + shift)).collect();
        for (label, v) in voices {
            let a = render(song_end(&song), |t| note_at(&song, t).map(|n| n.2), AMP, v);
            let (voiced, ok) = frame_quality(&song, &a);
            for d in [Difficulty::Normal, Difficulty::Hard] {
                let b = score_audio(&song, d, &a);
                eprintln!("| {label} | {reg} | {d:?} | {:.1} | {:.1} | {voiced:.1} | {ok:.1} |", b.v1.score, b.v2.score);
            }
        }
    }
}

#[test]
fn p6_exploits() {
    let song = regression_song();
    header("P6 explorações (só Scorer, frames ideais, Normal)");
    let best = (0..12)
        .map(|pc| {
            (
                pc,
                score_direct(&song, Difficulty::Normal, |_| Some(60.0 + pc as f64)),
            )
        })
        .max_by(|a, b| a.1.v2.score.total_cmp(&b.1.v2.score));
    if let Some((pc, r)) = best {
        row(&format!("nota fixa contínua (melhor classe no v2: +{pc} st)"), &r);
    }
    for period in [200.0, 400.0, 800.0, 1600.0] {
        // Siren: triangle wave over one octave (covers every pitch class twice per period).
        let r = score_direct(&song, Difficulty::Normal, |t| {
            let x = (t % period) / period;
            Some(60.0 + 12.0 * (1.0 - (2.0 * x - 1.0).abs()))
        });
        row(&format!("sirene de 1 oitava, período {period} ms"), &r);
    }
    for period in [100.0, 300.0] {
        let r = score_direct(&song, Difficulty::Normal, |t| {
            Some(60.0 + 12.0 * (t % period) / period)
        });
        row(&format!("dente de serra 1 oitava, período {period} ms"), &r);
    }
    let r = score_direct(&song, Difficulty::Normal, |t| {
        note_at(&song, t).map(|n| n.2 + 2.0)
    });
    row(
        "referência: tudo +200 cents (tentativa real, tom errado)",
        &r,
    );

    header("P6b sirene pelo pipeline (áudio, senoide)");
    for period in [400.0, 800.0] {
        let a = render(
            song_end(&song),
            |t| {
                let x = (t % period) / period;
                Some(60.0 + 12.0 * (1.0 - (2.0 * x - 1.0).abs()))
            },
            AMP,
            SINE,
        );
        row(
            &format!("sirene período {period} ms"),
            &score_audio(&song, Difficulty::Normal, &a),
        );
    }
}

#[test]
fn p7_guide_vocal_leak() {
    // User silent; the speakers play the video with a guide vocal and the mic picks it up.
    let song = regression_song();
    header("P7 vazamento da voz-guia (usuário em silêncio, gate padrão −50 dBFS)");
    for atten_db in [20.0, 30.0, 35.0, 40.0] {
        let a = render(
            song_end(&song),
            |t| note_at(&song, t).map(|n| n.2),
            AMP * 10f64.powf(-atten_db / 20.0),
            VOWEL,
        );
        row(
            &format!("voz-guia −{atten_db} dB"),
            &score_audio(&song, Difficulty::Normal, &a),
        );
    }
}

#[test]
fn p8_missing_frames() {
    let song = regression_song();
    header("P8 frames ausentes (só Scorer): denominador = frames recebidos");
    // Only the first 100 ms of each note reach the scorer (e.g. capture stalls), sung perfectly.
    let frames: Vec<VoiceFrame> = (0..(song_end(&song) / 10.0) as usize)
        .map(|i| i as f64 * 10.0)
        .filter(|&t| song.iter().any(|&(s, _, _)| t >= s - 20.0 && t < s + 100.0))
        .map(|t| {
            let m = note_at(&song, t).map(|n| n.2);
            VoiceFrame {
                t_ms: t,
                f0_hz: m.map(|m| midi_hz(m) as f32),
                confidence: 0.95,
                voiced: m.is_some(),
            }
        })
        .collect();
    row(
        "só os 100 ms iniciais de cada nota chegam",
        &score_frames(&song, Difficulty::Normal, &frames),
    );
}

// ======================= v2 re-review (T-006-08) =======================

/// The T-003-05 test song: legato, repeated notes, a 150 ms note, a rest, a fifth leap.
fn test_song() -> Song {
    let json = include_str!("../../../apps/desktop/e2e/tools/test-song.mon-melody.json");
    scoring_engine::parse_reference_json(json)
        .expect("test song")
        .notes
        .iter()
        .map(|n| (n.start_ms as f64, n.dur_ms as f64, n.midi as f64))
        .collect()
}

/// Amateur singer: realistic voice, slow random drift up to ±`drift` cents, vibrato ±`vib` cents at 5.5 Hz,
/// constant bias, and optionally a scoop (each note starts `scoop` cents low, gliding up over 100 ms).
fn amateur(song: &Song, d: Difficulty, drift: f64, vib: f64, bias: f64, scoop: f64) -> Both {
    let v = Voice { drift_cents: drift, ..HUMAN };
    let a = render(
        song_end(song),
        |t| {
            note_at(song, t).map(|(s, _, m)| {
                let glide = scoop * (1.0 - ((t - s) / 100.0).min(1.0));
                m + (bias - glide + vib * (TAU * 5.5 * t / 1000.0).sin()) / 100.0
            })
        },
        AMP,
        v,
    );
    score_audio(song, d, &a)
}

#[test]
fn p9_amateur_curve() {
    for (name, song) in [("música de teste (T-003-05)", test_song()), ("melodia da regressão", regression_song())] {
        eprintln!("\n### P9a amador, Normal, {name}: v1 → v2 (linhas: drift até ±D; colunas: vibrato ±V a 5,5 Hz)");
        eprintln!("| drift / vibrato | 0 | ±50 | ±75 | ±100 |\n|---|---|---|---|---|");
        for drift in [0.0, 15.0, 25.0, 35.0] {
            let cells: Vec<String> = [0.0, 50.0, 75.0, 100.0]
                .iter()
                .map(|&vib| {
                    let b = amateur(&song, Difficulty::Normal, drift, vib, 0.0, 0.0);
                    format!("{:.1} → {:.1}", b.v1.score, b.v2.score)
                })
                .collect();
            eprintln!("| ±{drift} | {} |", cells.join(" | "));
        }
    }
    let song = test_song();
    eprintln!("\n### P9b desvio constante + drift ±20 + vibrato ±50 (música de teste): v1 → v2");
    eprintln!("| desvio | Easy | Normal | Hard |\n|---|---|---|---|");
    for bias in [0.0, 10.0, 20.0, 30.0, -20.0, -40.0] {
        let cells: Vec<String> = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard]
            .iter()
            .map(|&d| {
                let b = amateur(&song, d, 20.0, 50.0, bias, 0.0);
                format!("{:.1} → {:.1}", b.v1.score, b.v2.score)
            })
            .collect();
        eprintln!("| {bias:+} c | {} |", cells.join(" | "));
    }
    header("P9c ataque por baixo (scoop, sobe em 100 ms) + drift ±20 + vibrato ±50, música de teste, Normal");
    for scoop in [0.0, 100.0, 150.0, 250.0] {
        row(&format!("scoop {scoop} c"), &amateur(&song, Difficulty::Normal, 20.0, 50.0, 0.0, scoop));
    }
}

#[test]
fn p10_monotonic_constant_detune() {
    let song = regression_song();
    for d in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
        let scores: Vec<(f64, f32, f32)> = (0..=20)
            .map(|k| {
                let c = k as f64 * 10.0;
                let b = score_direct(&song, d, |t| note_at(&song, t).map(|n| n.2 + c / 100.0));
                (c, b.v1.score, b.v2.score)
            })
            .collect();
        let jump = |sel: fn(&(f64, f32, f32)) -> f32| {
            scores.windows(2).map(|w| sel(&w[0]) - sel(&w[1])).fold(f32::MIN, f32::max)
        };
        let rising = scores.windows(2).filter(|w| w[1].2 > w[0].2).count();
        eprintln!(
            "\n### P10 {d:?}: desvio constante 0..200 c (frames ideais); maior queda entre vizinhos v1 {:.1} / v2 {:.1}; subidas no v2: {rising}",
            jump(|x| x.1),
            jump(|x| x.2)
        );
        eprintln!("{}", scores.iter().map(|(c, a, b)| format!("{c}:{a:.1}→{b:.1}")).collect::<Vec<_>>().join("  "));
    }
    header("P10b Normal pelo pipeline (senoide)");
    for c in [40.0, 60.0, 80.0, 90.0, 95.0, 100.0, 105.0, 110.0, 130.0] {
        let a = render(song_end(&song), |t| note_at(&song, t).map(|n| n.2 + c / 100.0), AMP, SINE);
        row(&format!("+{c} c"), &score_audio(&song, Difficulty::Normal, &a));
    }
}

#[test]
fn p11_gate_exploits() {
    let song = regression_song();
    header("P11 gate contínuo: cantar parte de cada nota (frames ideais, Normal)");
    for (label, frac) in [
        ("só a 1ª metade de cada nota, afinada", 0.5),
        ("só os primeiros 30 %", 0.3),
        ("só os primeiros 150 ms", 0.15),
    ] {
        let b = score_direct(&song, Difficulty::Normal, |t| note_at(&song, t).filter(|n| t < n.0 + frac * n.1).map(|n| n.2));
        row(label, &b);
    }
    let b = score_direct(&song, Difficulty::Normal, |t| {
        note_at(&song, t).map(|n| n.2 + if (t / 62.5) as i64 % 2 == 0 { 0.0 } else { 2.0 })
    });
    row("trinado alvo / +2 st a 8 Hz", &b);
    let b = score_direct(&song, Difficulty::Normal, |t| note_at(&song, t).filter(|_| (t / 100.0) as i64 % 2 == 0).map(|n| n.2));
    row("afinado, mas picotado (100 ms sim / 100 ms não)", &b);
    let b = score_direct(&song, Difficulty::Normal, |t| {
        note_at(&song, t).map(|n| if t < n.0 + n.1 / 2.0 { n.2 } else { n.2 + 7.0 })
    });
    row("1ª metade certa, 2ª metade uma quinta acima", &b);
}

#[test]
fn p12_onset_rules() {
    header("P12 regra de onset (mais perto deste alvo que do anterior + guarda de 25 ms), pipeline senoide, Normal");
    let pitches = [60.0, 61.0, 60.0, 61.0, 60.0, 61.0, 60.0, 61.0, 60.0, 61.0];
    let song = legato(&pitches, 500.0);
    for glide in [0.0, 60.0, 120.0] {
        let a = render(
            song_end(&song),
            |t| {
                let i = song.iter().position(|&(s, d, _)| t >= s && t < s + d)?;
                let (s, _, m) = song[i];
                let from = if i == 0 { m } else { song[i - 1].2 };
                Some(if glide == 0.0 { m } else { from + (m - from) * ((t - s) / glide).min(1.0) })
            },
            AMP,
            SINE,
        );
        row(&format!("semitons ligados, portamento {glide} ms"), &score_audio(&song, Difficulty::Normal, &a));
    }
    let song = legato(&[60.0, 72.0, 60.0, 72.0, 60.0, 72.0], 500.0);
    let a = render(song_end(&song), |t| note_at(&song, t).map(|n| n.2), AMP, SINE);
    row("saltos de oitava ligados (C4/C5)", &score_audio(&song, Difficulty::Normal, &a));
    let song = legato(&[60.0, 62.0, 64.0, 62.0, 60.0, 62.0, 64.0, 62.0, 60.0, 62.0], 500.0);
    for hold in [0.0, 80.0, 150.0] {
        let a = render(
            song_end(&song),
            |t| {
                let i = song.iter().position(|&(s, d, _)| t >= s && t < s + d)?;
                let (s, _, m) = song[i];
                Some(if i > 0 && t < s + hold { song[i - 1].2 } else { m })
            },
            AMP,
            SINE,
        );
        row(&format!("tons ligados, segura a nota anterior {hold} ms"), &score_audio(&song, Difficulty::Normal, &a));
    }
    let song = regression_song();
    for delay in [-300.0, -200.0, -150.0, 150.0, 200.0, 300.0] {
        let a = render(song_end(&song), |t| note_at(&song, t - delay).map(|n| n.2), AMP, SINE);
        row(&format!("deslocado {delay:+} ms"), &score_audio(&song, Difficulty::Normal, &a));
    }
    for early in [190.0, 400.0] {
        let a = render(
            song_end(&song),
            |t| song.iter().find(|&&(s, d, _)| t >= s - early && t < s + d).map(|n| n.2),
            AMP,
            SINE,
        );
        row(&format!("entra {early} ms antes e segura até o fim"), &score_audio(&song, Difficulty::Normal, &a));
    }
}

#[test]
fn p13_transition_zones() {
    header("P13 zonas de transição (pipeline senoide, Normal, 20 notas C4/E4 ligadas)");
    let pitches: Vec<f64> = (0..20).map(|i| if i % 2 == 0 { 60.0 } else { 64.0 }).collect();
    for dur in [50.0, 100.0, 150.0] {
        let song = legato(&pitches, dur);
        for late in [0.0, 25.0, 50.0] {
            let a = render(song_end(&song), |t| note_at(&song, t - late).map(|n| n.2), AMP, SINE);
            row(&format!("{dur} ms ligadas, cantadas {late} ms atrasadas"), &score_audio(&song, Difficulty::Normal, &a));
        }
    }
}

#[test]
fn p14_fixed_denominator() {
    let song = regression_song();
    header("P14 denominador fixo: frames que não chegam (frames ideais, Normal, canto perfeito)");
    let ideal = |keep: &dyn Fn(usize, f64) -> bool| -> Vec<VoiceFrame> {
        (0..(song_end(&song) / 10.0) as usize)
            .map(|i| (i, i as f64 * 10.0))
            .filter(|&(i, t)| keep(i, t))
            .map(|(_, t)| {
                let m = note_at(&song, t).map(|n| n.2);
                VoiceFrame { t_ms: t, f0_hz: m.map(|m| midi_hz(m) as f32), confidence: 0.95, voiced: m.is_some() }
            })
            .collect()
    };
    row("todos os frames", &score_frames(&song, Difficulty::Normal, &ideal(&|_, _| true)));
    let mut rng = Rng(0xD20B);
    let drop: Vec<bool> = (0..2000).map(|_| rng.next() < -0.9).collect(); // ~5 % random drops
    row("~5 % dos frames perdidos ao acaso", &score_frames(&song, Difficulty::Normal, &ideal(&|i, _| !drop[i])));
    for (pct, thr) in [(10, -0.8), (15, -0.7)] {
        let mut rng = Rng(0xD20B + pct as u64);
        let drop: Vec<bool> = (0..2000).map(|_| rng.next() < thr).collect();
        row(&format!("~{pct} % dos frames perdidos ao acaso (déficit do driver)"), &score_frames(&song, Difficulty::Normal, &ideal(&|i, _| !drop[i])));
    }
    row("rajada de 200 ms perdida a cada 5 s", &score_frames(&song, Difficulty::Normal, &ideal(&|_, t| t % 5000.0 >= 200.0)));
    row(
        "só os 100 ms iniciais de cada nota chegam (P8)",
        &score_frames(&song, Difficulty::Normal, &ideal(&|_, t| song.iter().any(|&(s, _, _)| t >= s - 20.0 && t < s + 100.0))),
    );
}
