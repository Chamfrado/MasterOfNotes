//! Microphone capture and voice analysis (pitch, VAD, level). Owner: Agent 2. Spec: SPEC-004.
//!
//! Analysis parameters (SPEC-004 R3–R5): mono input, window ≈ 42.7 ms
//! (2048 samples @ 48 kHz, proportional at other rates), hop 10 ms, YIN with
//! threshold 0.15 + parabolic interpolation, 70–1100 Hz. Volume never changes
//! pitch or confidence; it only matters through the noise gate.

mod yin;

use serde::{Deserialize, Serialize};

pub const YIN_THRESHOLD: f32 = 0.15;
pub const MIN_F0_HZ: f32 = 70.0;
pub const MAX_F0_HZ: f32 = 1100.0;
/// Frames below this YIN confidence report `f0_hz = None`.
pub const MIN_CONFIDENCE: f32 = 0.6;
/// |sample| at or above this marks the frame as clipped.
pub const CLIP_LEVEL: f32 = 0.99;
/// Floor reported for digital silence.
pub const SILENCE_DBFS: f32 = -120.0;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("audio device error: {0}")]
    Device(String),
}

/// Analysis window length in samples (2048 @ 48 kHz).
pub fn window_len(sample_rate: u32) -> usize {
    ((2048 * sample_rate as u64 + 24_000) / 48_000) as usize
}

/// Hop length in samples (10 ms).
pub fn hop_len(sample_rate: u32) -> usize {
    ((sample_rate as u64 + 50) / 100) as usize
}

/// One analysis frame per hop (10 ms). Volume does NOT influence pitch/confidence.
/// The frame covers samples `[sample_index, sample_index + window_len)`; its
/// time centre is `sample_index + window_len / 2`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisFrame {
    /// Index of the frame's first sample in the stream (monotonic).
    pub sample_index: u64,
    pub sample_rate: u32,
    /// `None` = no detectable pitch (silence, noise, fricative).
    pub f0_hz: Option<f32>,
    /// 0..1 (YIN: 1 − aperiodicity).
    pub confidence: f32,
    pub rms_dbfs: f32,
    /// VAD: above the noise gate AND with reliable pitch.
    pub voiced: bool,
    /// Some |sample| ≥ 0.99 within this frame's hop.
    pub clipped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureConfig {
    /// From noise calibration.
    pub noise_floor_dbfs: f32,
    pub gate_margin_db: f32,
}

impl Default for CaptureConfig {
    /// Uncalibrated default: a quiet room (−60 dBFS) with the 10 dB margin of SPEC-004 R4.
    fn default() -> Self {
        Self {
            noise_floor_dbfs: -60.0,
            gate_margin_db: 10.0,
        }
    }
}

/// Streaming analyzer: feed mono samples in any chunk size, get one frame per hop.
/// Shared by [`analyze_buffer`] and the live capture so both run the same code.
pub(crate) struct Analyzer {
    sample_rate: u32,
    cfg: CaptureConfig,
    window: usize,
    hop: usize,
    buf: Vec<f32>,
    buf_start: u64,
    yin: yin::Yin,
}

impl Analyzer {
    pub(crate) fn new(sample_rate: u32, cfg: CaptureConfig) -> Self {
        let window = window_len(sample_rate);
        Self {
            sample_rate,
            cfg,
            window,
            hop: hop_len(sample_rate),
            buf: Vec::with_capacity(window * 4),
            buf_start: 0,
            yin: yin::Yin::new(sample_rate),
        }
    }

    pub(crate) fn push(&mut self, samples: &[f32], mut on_frame: impl FnMut(AnalysisFrame)) {
        self.buf.extend_from_slice(samples);
        let mut pos = 0;
        while pos + self.window <= self.buf.len() {
            let frame = self.analyze(pos);
            on_frame(frame);
            pos += self.hop;
        }
        self.buf.drain(..pos);
        self.buf_start += pos as u64;
    }

    fn analyze(&mut self, pos: usize) -> AnalysisFrame {
        let w = &self.buf[pos..pos + self.window];
        let energy: f64 = w.iter().map(|&s| (s as f64) * (s as f64)).sum();
        let rms = (energy / w.len() as f64).sqrt();
        let rms_dbfs = if rms > 0.0 {
            ((20.0 * rms.log10()) as f32).max(SILENCE_DBFS)
        } else {
            SILENCE_DBFS
        };
        let clipped = w[..self.hop].iter().any(|s| s.abs() >= CLIP_LEVEL);
        let (f0_hz, confidence) = self.yin.detect(w);
        let above_gate = rms_dbfs >= self.cfg.noise_floor_dbfs + self.cfg.gate_margin_db;
        AnalysisFrame {
            sample_index: self.buf_start + pos as u64,
            sample_rate: self.sample_rate,
            f0_hz,
            confidence,
            rms_dbfs,
            voiced: above_gate && f0_hz.is_some(),
            clipped,
        }
    }
}

/// Pure offline analysis of a mono buffer (same code path as the live capture).
pub fn analyze_buffer(
    samples: &[f32],
    sample_rate: u32,
    cfg: &CaptureConfig,
) -> Vec<AnalysisFrame> {
    let mut frames = Vec::with_capacity(samples.len() / hop_len(sample_rate).max(1) + 1);
    Analyzer::new(sample_rate, *cfg).push(samples, |f| frames.push(f));
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_in_odd_chunks_matches_one_shot() {
        let s: Vec<f32> = (0..48_000)
            .map(|i| (i as f32 * 0.037).sin() * 0.3)
            .collect();
        let whole = analyze_buffer(&s, 48_000, &CaptureConfig::default());
        let mut chunked = Vec::new();
        let mut a = Analyzer::new(48_000, CaptureConfig::default());
        for c in s.chunks(333) {
            a.push(c, |f| chunked.push(f));
        }
        assert_eq!(whole, chunked);
    }
}
