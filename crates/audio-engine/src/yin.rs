//! YIN fundamental-frequency estimator (de Cheveigné & Kawahara, 2002).
//! Steps: difference function → cumulative mean normalized difference (CMND) →
//! absolute threshold (global minimum as fallback) → parabolic interpolation.

use crate::{MAX_F0_HZ, MIN_CONFIDENCE, MIN_F0_HZ, YIN_THRESHOLD};

pub(crate) struct Yin {
    sample_rate: f32,
    tau_min: usize,
    tau_max: usize,
    /// CMND, reused between frames (no per-frame allocation).
    cmnd: Vec<f32>,
}

impl Yin {
    pub(crate) fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        let tau_min = ((sr / MAX_F0_HZ).floor() as usize).max(2);
        let tau_max = (sr / MIN_F0_HZ).ceil() as usize;
        Self {
            sample_rate: sr,
            tau_min,
            tau_max,
            cmnd: vec![0.0; tau_max + 2],
        }
    }

    /// Returns `(f0, confidence)`; `f0` is `None` when the frame has no reliable
    /// periodicity in 70–1100 Hz. Confidence = 1 − CMND at the chosen lag, so it
    /// is independent of signal level.
    pub(crate) fn detect(&mut self, x: &[f32]) -> (Option<f32>, f32) {
        let last_tau = self.tau_max + 1; // +1 so the parabola has a right neighbour
        if x.len() <= last_tau + 1 {
            return (None, 0.0);
        }
        let w = x.len() - last_tau;

        self.cmnd[0] = 1.0;
        let mut running = 0.0f64;
        for tau in 1..=last_tau {
            let d = diff(&x[..w], &x[tau..tau + w]);
            running += d;
            self.cmnd[tau] = if running > 0.0 {
                (d * tau as f64 / running) as f32
            } else {
                1.0 // digital silence: no periodicity
            };
        }

        let c = &self.cmnd;
        let mut tau = match (self.tau_min..=self.tau_max).find(|&t| c[t] < YIN_THRESHOLD) {
            Some(t) => t,
            None => (self.tau_min..=self.tau_max)
                .min_by(|&a, &b| c[a].total_cmp(&c[b]))
                .unwrap_or(self.tau_min),
        };
        // Walk down to the bottom of the dip that crossed the threshold.
        while tau < self.tau_max && c[tau + 1] < c[tau] {
            tau += 1;
        }

        let (a, b, cc) = (c[tau - 1], c[tau], c[tau + 1]);
        let denom = a - 2.0 * b + cc;
        let shift = if denom.abs() > f32::EPSILON {
            (0.5 * (a - cc) / denom).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let confidence = (1.0 - b).clamp(0.0, 1.0);
        let f0 = self.sample_rate / (tau as f32 + shift);
        let valid = confidence >= MIN_CONFIDENCE && (MIN_F0_HZ..=MAX_F0_HZ).contains(&f0);
        (valid.then_some(f0), confidence)
    }
}

/// Σ (a[j] − b[j])², accumulated in 8 independent lanes so the compiler can
/// vectorise it; the final sum is in f64 for precision. Deterministic order.
fn diff(a: &[f32], b: &[f32]) -> f64 {
    let mut lanes = [0.0f32; 8];
    let ((ca, ra), (cb, rb)) = (a.as_chunks::<8>(), b.as_chunks::<8>());
    let tail: f32 = ra.iter().zip(rb).map(|(x, y)| (x - y) * (x - y)).sum();
    for (xa, xb) in ca.iter().zip(cb) {
        for i in 0..8 {
            let d = xa[i] - xb[i];
            lanes[i] += d * d;
        }
    }
    lanes.iter().map(|&v| v as f64).sum::<f64>() + tail as f64
}
