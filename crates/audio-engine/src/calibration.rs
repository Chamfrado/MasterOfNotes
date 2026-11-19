//! Noise calibration (SPEC-004 R6). Pure functions over analysis frames, so the
//! UI flow (N s of silence, then a voice test) can be driven by any capture.

use crate::{AnalysisFrame, CaptureConfig, SILENCE_DBFS};
use serde::Serialize;

/// Noise floor above this is too noisy to score reliably.
const NOISY_DBFS: f32 = -40.0;
/// More than this fraction of clipped frames is a problem.
const MAX_CLIPPED_RATIO: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CalibrationIssue {
    Noisy,
    Clipping,
    TooQuiet,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseCalibration {
    /// 90th percentile of the frame RMS during silence. Use as `CaptureConfig::noise_floor_dbfs`.
    pub noise_floor_dbfs: f32,
    pub clipped_ratio: f32,
    pub issues: Vec<CalibrationIssue>,
}

/// Nearest-rank 90th percentile of the frame RMS (dBFS); `SILENCE_DBFS` if empty.
fn p90_dbfs(frames: &[AnalysisFrame]) -> f32 {
    let mut v: Vec<f32> = frames.iter().map(|f| f.rms_dbfs).collect();
    if v.is_empty() {
        return SILENCE_DBFS;
    }
    v.sort_by(f32::total_cmp);
    let rank = (v.len() * 9).div_ceil(10);
    v[rank.saturating_sub(1)]
}

fn clipped_ratio(frames: &[AnalysisFrame]) -> f32 {
    if frames.is_empty() {
        return 0.0;
    }
    frames.iter().filter(|f| f.clipped).count() as f32 / frames.len() as f32
}

/// Step 1: the user stays silent for a few seconds.
pub fn calibrate_noise(frames: &[AnalysisFrame]) -> NoiseCalibration {
    let noise_floor_dbfs = p90_dbfs(frames);
    let clipped_ratio = clipped_ratio(frames);
    let mut issues = Vec::new();
    if noise_floor_dbfs > NOISY_DBFS {
        issues.push(CalibrationIssue::Noisy);
    }
    if clipped_ratio > MAX_CLIPPED_RATIO {
        issues.push(CalibrationIssue::Clipping);
    }
    NoiseCalibration {
        noise_floor_dbfs,
        clipped_ratio,
        issues,
    }
}

/// Step 2: the user sings/speaks. `TooQuiet` if the voice (90th percentile RMS)
/// does not clear the gate; `Clipping` if more than 1% of frames clip.
pub fn check_voice(frames: &[AnalysisFrame], cfg: &CaptureConfig) -> Vec<CalibrationIssue> {
    let mut issues = Vec::new();
    if p90_dbfs(frames) < cfg.noise_floor_dbfs + cfg.gate_margin_db {
        issues.push(CalibrationIssue::TooQuiet);
    }
    if clipped_ratio(frames) > MAX_CLIPPED_RATIO {
        issues.push(CalibrationIssue::Clipping);
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(rms_dbfs: f32, clipped: bool) -> AnalysisFrame {
        AnalysisFrame {
            sample_index: 0,
            sample_rate: 48_000,
            f0_hz: None,
            confidence: 0.0,
            rms_dbfs,
            voiced: false,
            clipped,
        }
    }

    #[test]
    fn noise_floor_is_the_90th_percentile() {
        // −70..−61 dBFS (10 frames): p90 (nearest rank 9) = −62.
        let frames: Vec<_> = (0..10).map(|i| frame(-70.0 + i as f32, false)).collect();
        let c = calibrate_noise(&frames);
        assert_eq!(c.noise_floor_dbfs, -62.0);
        assert!(c.issues.is_empty());
    }

    #[test]
    fn noisy_and_clipping_verdicts() {
        let mut frames = vec![frame(-35.0, false); 99];
        frames.push(frame(-35.0, true)); // exactly 1%: not an issue
        assert_eq!(
            calibrate_noise(&frames).issues,
            vec![CalibrationIssue::Noisy]
        );
        frames.push(frame(-35.0, true));
        assert_eq!(
            calibrate_noise(&frames).issues,
            vec![CalibrationIssue::Noisy, CalibrationIssue::Clipping]
        );
    }

    #[test]
    fn voice_test_must_clear_the_gate() {
        let cfg = CaptureConfig {
            noise_floor_dbfs: -55.0,
            gate_margin_db: 10.0,
        };
        assert_eq!(
            check_voice(&vec![frame(-50.0, false); 50], &cfg),
            vec![CalibrationIssue::TooQuiet]
        );
        assert!(check_voice(&vec![frame(-30.0, false); 50], &cfg).is_empty());
        assert_eq!(check_voice(&[], &cfg), vec![CalibrationIssue::TooQuiet]);
    }

    #[test]
    fn empty_calibration_is_safe() {
        let c = calibrate_noise(&[]);
        assert_eq!((c.noise_floor_dbfs, c.clipped_ratio), (SILENCE_DBFS, 0.0));
    }
}
