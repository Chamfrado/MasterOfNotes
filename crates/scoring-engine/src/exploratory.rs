//! SPEC-006 item 9: without a reference there is no score, only exploratory,
//! non-competitive metrics. Never feed these into rankings.

use crate::VoiceFrame;
use crate::scorer::median;
use serde::Serialize;

/// Voiced runs shorter than this are ignored for stability.
const MIN_RUN_MS: f64 = 200.0;
/// A gap larger than this between consecutive frames splits a run.
const MAX_FRAME_GAP_MS: f64 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExploratoryMetrics {
    /// Fraction of frames that are voiced (0..1).
    pub voiced_ratio: f32,
    /// 1 − min(1, MAD / 100 cents), where MAD is the median absolute deviation of
    /// pitch from its run median, pooled over continuous voiced runs ≥ 200 ms.
    /// 0 when there is no such run.
    pub pitch_stability: f32,
}

pub fn exploratory_metrics(frames: &[VoiceFrame]) -> ExploratoryMetrics {
    let voiced = |f: &VoiceFrame| f.voiced && f.f0_hz.is_some_and(|hz| hz > 0.0);
    let voiced_ratio = if frames.is_empty() {
        0.0
    } else {
        frames.iter().filter(|f| voiced(f)).count() as f32 / frames.len() as f32
    };

    let mut deviations = Vec::new();
    let mut run: Vec<(f64, f64)> = Vec::new(); // (t_ms, cents)
    let mut close = |run: &mut Vec<(f64, f64)>| {
        if let (Some(first), Some(last)) = (run.first(), run.last())
            && last.0 - first.0 >= MIN_RUN_MS
        {
            let mut cents: Vec<f64> = run.iter().map(|r| r.1).collect();
            let m = median(&mut cents).unwrap_or(0.0); // run is non-empty
            deviations.extend(cents.iter().map(|c| (c - m).abs()));
        }
        run.clear();
    };
    for f in frames {
        let contiguous = run.last().is_none_or(|l| f.t_ms - l.0 <= MAX_FRAME_GAP_MS);
        if !voiced(f) || !contiguous {
            close(&mut run);
        }
        if let Some(hz) = f.f0_hz.filter(|_| voiced(f)) {
            run.push((f.t_ms, 1200.0 * (hz as f64 / 440.0).log2()));
        }
    }
    close(&mut run);

    let pitch_stability =
        median(&mut deviations).map_or(0.0, |mad| (1.0 - (mad / 100.0).min(1.0)) as f32);
    ExploratoryMetrics {
        voiced_ratio,
        pitch_stability,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(n: usize, hz: impl Fn(usize) -> Option<f32>) -> Vec<VoiceFrame> {
        (0..n)
            .map(|i| {
                let f0 = hz(i);
                VoiceFrame {
                    t_ms: i as f64 * 10.0,
                    f0_hz: f0,
                    confidence: 0.9,
                    voiced: f0.is_some(),
                }
            })
            .collect()
    }

    #[test]
    fn stable_tone_is_fully_stable() {
        let m = exploratory_metrics(&frames(300, |_| Some(220.0)));
        assert_eq!(m.voiced_ratio, 1.0);
        assert!(m.pitch_stability > 0.999, "{m:?}");
    }

    #[test]
    fn continuous_glissando_is_much_less_stable() {
        // One octave up over 3 s: deviations from the run median reach ±600 cents.
        let m = exploratory_metrics(&frames(300, |i| Some(220.0 * 2f32.powf(i as f32 / 300.0))));
        assert!(m.pitch_stability < 0.1, "{m:?}");
    }

    #[test]
    fn small_vibrato_costs_a_little() {
        // ±30 cents sine vibrato: MAD ≈ 21 cents → stability ≈ 0.79.
        let m = exploratory_metrics(&frames(300, |i| {
            let c = 30.0 * (std::f32::consts::TAU * 6.0 * i as f32 / 100.0).sin();
            Some(220.0 * 2f32.powf(c / 1200.0))
        }));
        assert!((0.7..0.9).contains(&m.pitch_stability), "{m:?}");
    }

    #[test]
    fn silence_and_short_blips() {
        let silent = exploratory_metrics(&frames(100, |_| None));
        assert_eq!((silent.voiced_ratio, silent.pitch_stability), (0.0, 0.0));
        // 150 ms voiced blips separated by silence never form a ≥ 200 ms run.
        let blips = exploratory_metrics(&frames(400, |i| (i % 40 < 15).then_some(300.0)));
        assert_eq!(blips.pitch_stability, 0.0);
        assert!((blips.voiced_ratio - 0.375).abs() < 1e-6);
        assert_eq!(exploratory_metrics(&[]).voiced_ratio, 0.0);
    }
}
