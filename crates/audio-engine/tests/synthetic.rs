//! Synthetic-signal tests for the analysis pipeline (SPEC-004 criteria).
//! All signals are generated in code: reproducible, no binaries, no licensing.

use audio_engine::{AnalysisFrame, CaptureConfig, analyze_buffer, window_len};
use std::f64::consts::TAU;

const SR: u32 = 48_000;

/// Peak amplitude of a sine whose RMS level is `dbfs`.
fn sine_amp(dbfs: f64) -> f64 {
    10f64.powf(dbfs / 20.0) * 2f64.sqrt()
}

fn sine(freq: f64, amp: f64, secs: f64, sr: u32) -> Vec<f32> {
    let n = (secs * sr as f64) as usize;
    (0..n)
        .map(|i| (amp * (TAU * freq * i as f64 / sr as f64).sin()) as f32)
        .collect()
}

/// Voice-like tone: harmonics 1..=10 with 1/k amplitude, peak-normalised to `amp`.
fn harmonic(freq: f64, amp: f64, secs: f64) -> Vec<f32> {
    let n = (secs * SR as f64) as usize;
    let raw: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            (1..=10)
                .map(|k| (TAU * freq * k as f64 * t).sin() / k as f64)
                .sum()
        })
        .collect();
    let peak = raw.iter().fold(0f64, |m, v| m.max(v.abs()));
    raw.iter().map(|v| (v / peak * amp) as f32).collect()
}

/// Uniform white noise from a fixed-seed xorshift (no `rand` dependency).
fn white_noise(amp: f32, secs: f64, mut seed: u64) -> Vec<f32> {
    let n = (secs * SR as f64) as usize;
    (0..n)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let u = (seed >> 11) as f32 / (1u64 << 53) as f32; // [0, 1)
            amp * (2.0 * u - 1.0)
        })
        .collect()
}

fn analyze(samples: &[f32]) -> Vec<AnalysisFrame> {
    analyze_buffer(samples, SR, &CaptureConfig::default())
}

fn fraction(frames: &[AnalysisFrame], pred: impl Fn(&AnalysisFrame) -> bool) -> f64 {
    frames.iter().filter(|f| pred(f)).count() as f64 / frames.len() as f64
}

fn within(f: &AnalysisFrame, hz: f32, rel: f32) -> bool {
    f.f0_hz.is_some_and(|f0| (f0 - hz).abs() <= hz * rel)
}

#[test]
fn sine_440_is_within_2hz_in_95_percent_of_frames() {
    let frames = analyze(&sine(440.0, sine_amp(-12.0), 2.0, SR));
    assert!(frames.len() > 190);
    let ok = fraction(&frames, |f| {
        f.f0_hz.is_some_and(|f0| (438.0..=442.0).contains(&f0))
    });
    assert!(ok >= 0.95, "440 Hz ok fraction = {ok}");
    assert!(fraction(&frames, |f| f.voiced) >= 0.95);
}

#[test]
fn sine_at_44100_hz_sample_rate() {
    let frames = analyze_buffer(
        &sine(440.0, 0.5, 1.0, 44_100),
        44_100,
        &CaptureConfig::default(),
    );
    let ok = fraction(&frames, |f| {
        f.f0_hz.is_some_and(|f0| (438.0..=442.0).contains(&f0))
    });
    assert!(ok >= 0.95, "ok fraction = {ok}");
}

#[test]
fn sines_across_the_vocal_range_are_within_one_percent() {
    for hz in [75.0, 98.0, 146.83, 261.63, 392.0, 659.26, 987.77, 1080.0] {
        let frames = analyze(&sine(hz, 0.3, 1.0, SR));
        let ok = fraction(&frames, |f| within(f, hz as f32, 0.01));
        assert!(ok >= 0.95, "{hz} Hz ok fraction = {ok}");
    }
}

#[test]
fn tone_below_range_has_no_pitch() {
    let frames = analyze(&sine(40.0, 0.3, 1.0, SR));
    assert!(frames.iter().all(|f| f.f0_hz.is_none() && !f.voiced));
}

#[test]
fn tone_above_range_is_read_as_its_in_range_subharmonic() {
    // Known YIN limitation (measured): lags shorter than 1/1100 s are not searched,
    // so a 2 kHz tone is read at 2 periods = 1000 Hz. Harmless for scoring — the
    // octave fold of SPEC-006 maps it to the same pitch class — and nobody sings
    // at 2 kHz in a karaoke. Never reported outside 70–1100 Hz.
    let frames = analyze(&sine(2000.0, 0.3, 1.0, SR));
    assert!(fraction(&frames, |f| within(f, 1000.0, 0.01)) >= 0.95);
}

#[test]
fn harmonic_voice_110hz_has_no_octave_error() {
    let frames = analyze(&harmonic(110.0, 0.5, 2.0));
    let ok = fraction(&frames, |f| within(f, 110.0, 0.01));
    assert!(ok >= 0.95, "harmonic 110 Hz ok fraction = {ok}");
}

#[test]
fn harmonic_voice_without_fundamental_still_tracks_period() {
    // Missing fundamental (harmonics 2..=8): the perceived pitch is still 110 Hz.
    let n = SR as usize;
    let s: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            (2..=8)
                .map(|k| (TAU * 110.0 * k as f64 * t).sin() / k as f64)
                .sum::<f64>() as f32
                * 0.4
        })
        .collect();
    let ok = fraction(&analyze(&s), |f| within(f, 110.0, 0.01));
    assert!(ok >= 0.95, "missing-fundamental ok fraction = {ok}");
}

#[test]
fn octaves_are_distinguished() {
    for hz in [110.0, 220.0, 440.0, 880.0] {
        let frames = analyze(&harmonic(hz, 0.5, 1.0));
        let ok = fraction(&frames, |f| within(f, hz as f32, 0.01));
        assert!(ok >= 0.95, "octave {hz} Hz ok fraction = {ok}");
    }
}

#[test]
fn glissando_is_tracked() {
    // Exponential sweep 200 → 400 Hz over 2 s; compare with the instantaneous
    // frequency at each window centre.
    let (f1, f2, secs) = (200.0f64, 400.0f64, 2.0f64);
    let k = (f2 / f1).ln() / secs;
    let n = (secs * SR as f64) as usize;
    let s: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            (0.4 * (TAU * f1 * ((k * t).exp() - 1.0) / k).sin()) as f32
        })
        .collect();
    let frames = analyze(&s);
    let half = window_len(SR) as f64 / 2.0;
    let ok = fraction(&frames, |f| {
        let t = (f.sample_index as f64 + half) / SR as f64;
        within(f, (f1 * (k * t).exp()) as f32, 0.02)
    });
    assert!(ok >= 0.95, "glissando ok fraction = {ok}");
}

#[test]
fn digital_silence_is_unvoiced() {
    let frames = analyze(&vec![0.0; SR as usize * 2]);
    assert!(
        frames
            .iter()
            .all(|f| !f.voiced && f.f0_hz.is_none() && !f.clipped)
    );
    assert!(
        frames
            .iter()
            .all(|f| f.rms_dbfs == audio_engine::SILENCE_DBFS)
    );
}

#[test]
fn white_noise_is_unvoiced_in_99_percent_of_frames() {
    for seed in [1u64, 42, 0xC0FFEE] {
        let frames = analyze(&white_noise(0.3, 3.0, seed));
        let voiced = fraction(&frames, |f| f.voiced);
        assert!(voiced <= 0.01, "seed {seed}: voiced fraction = {voiced}");
    }
}

#[test]
fn clipping_is_flagged() {
    let hard: Vec<f32> = sine(440.0, 1.5, 0.5, SR)
        .iter()
        .map(|s| s.clamp(-1.0, 1.0))
        .collect();
    assert!(fraction(&analyze(&hard), |f| f.clipped) >= 0.99);
    assert!(
        analyze(&sine(440.0, 0.9, 0.5, SR))
            .iter()
            .all(|f| !f.clipped)
    );
}

#[test]
fn volume_does_not_change_pitch_or_voicing_above_the_gate() {
    let quiet = analyze(&sine(330.0, sine_amp(-30.0), 1.0, SR));
    let loud = analyze(&sine(330.0, sine_amp(-6.0), 1.0, SR));
    assert_eq!(quiet.len(), loud.len());
    for (q, l) in quiet.iter().zip(&loud) {
        assert_eq!(q.voiced, l.voiced);
        assert!(q.voiced);
        let (qf, lf) = (q.f0_hz.unwrap(), l.f0_hz.unwrap());
        assert!((qf - lf).abs() < 0.01, "{qf} vs {lf}");
        assert!((q.confidence - l.confidence).abs() < 1e-3);
        assert!((l.rms_dbfs - q.rms_dbfs - 24.0).abs() < 0.1);
    }
}

#[test]
fn below_the_gate_is_unvoiced_even_with_pitch() {
    let cfg = CaptureConfig {
        noise_floor_dbfs: -50.0,
        gate_margin_db: 10.0,
    };
    let frames = analyze_buffer(&sine(330.0, sine_amp(-45.0), 1.0, SR), SR, &cfg);
    assert!(frames.iter().all(|f| !f.voiced && f.f0_hz.is_some()));
}

#[test]
fn frames_are_hop_spaced_and_deterministic() {
    let s = sine(250.0, 0.4, 1.0, SR);
    let whole = analyze(&s);
    for (i, f) in whole.iter().enumerate() {
        assert_eq!(f.sample_index, i as u64 * 480);
        assert_eq!(f.sample_rate, SR);
    }
    assert_eq!(whole, analyze(&s));
}
