//! Latency measurement (SPEC-004 R7, T-004-07): pure parts. Clicks are played at
//! known times; transients (claps, or the click picked up from the speakers) are
//! detected in the microphone signal and paired with the clicks.

use serde::Serialize;

/// Envelope: RMS over 5 ms, evaluated every 1 ms.
const ENV_WINDOW_MS: usize = 5;
/// A transient rises at least this much within 5 ms...
const ONSET_RISE_DB: f32 = 15.0;
/// ...to at least this much above the noise floor.
const ONSET_ABOVE_FLOOR_DB: f32 = 20.0;
/// One transient per this interval (clap ring, room echo).
const REFRACTORY_MS: f64 = 150.0;
/// Pair a click with the first onset in [click − 100 ms, click + 600 ms].
const PAIR_EARLY_MS: f64 = 100.0;
const PAIR_LATE_MS: f64 = 600.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyResult {
    /// Median delay of the detected transients after their clicks.
    pub offset_ms: f32,
    /// Median absolute deviation of the delays.
    pub jitter_ms: f32,
    pub detected: u32,
    pub expected: u32,
}

fn level_dbfs(x: &[f32]) -> f32 {
    let e: f64 = x.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (e / x.len().max(1) as f64).sqrt();
    if rms > 0.0 {
        (20.0 * rms.log10()) as f32
    } else {
        crate::SILENCE_DBFS
    }
}

/// Transient start times (ms from the first sample), sample-accurate.
pub fn detect_onsets(samples: &[f32], sample_rate: u32, noise_floor_dbfs: f32) -> Vec<f64> {
    let ms = (sample_rate as usize / 1000).max(1);
    let win = ENV_WINDOW_MS * ms;
    if samples.len() < win {
        return Vec::new();
    }
    let env: Vec<f32> = (0..=(samples.len() - win) / ms)
        .map(|i| level_dbfs(&samples[i * ms..i * ms + win]))
        .collect();
    let mut onsets: Vec<f64> = Vec::new();
    for i in ENV_WINDOW_MS..env.len() {
        let rising = env[i] - env[i - ENV_WINDOW_MS] >= ONSET_RISE_DB;
        if !rising || env[i] < noise_floor_dbfs + ONSET_ABOVE_FLOOR_DB {
            continue;
        }
        // Sample-accurate start: first sample in the rise region reaching a
        // quarter of the local peak.
        let from = (i - ENV_WINDOW_MS) * ms;
        let to = (i * ms + win).min(samples.len());
        let peak = samples[from..to].iter().fold(0f32, |m, s| m.max(s.abs()));
        let start = samples[from..to]
            .iter()
            .position(|s| s.abs() >= 0.25 * peak)
            .map_or(from, |p| from + p);
        let t = start as f64 * 1000.0 / sample_rate as f64;
        if onsets.last().is_none_or(|&last| t - last >= REFRACTORY_MS) {
            onsets.push(t);
        }
    }
    onsets
}

/// Pairs each click with the first unused onset in its window; offset = median
/// delay, jitter = median absolute deviation. Both inputs in ms, sorted.
pub fn latency_from(onsets_ms: &[f64], clicks_ms: &[f64]) -> LatencyResult {
    let mut delays = Vec::new();
    let mut next = 0;
    for &c in clicks_ms {
        while next < onsets_ms.len() && onsets_ms[next] < c - PAIR_EARLY_MS {
            next += 1;
        }
        if next < onsets_ms.len() && onsets_ms[next] <= c + PAIR_LATE_MS {
            delays.push(onsets_ms[next] - c);
            next += 1;
        }
    }
    let offset = median(&mut delays.clone());
    let mut dev: Vec<f64> = delays.iter().map(|d| (d - offset).abs()).collect();
    LatencyResult {
        offset_ms: offset as f32,
        jitter_ms: median(&mut dev) as f32,
        detected: delays.len() as u32,
        expected: clicks_ms.len() as u32,
    }
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    let m = v.len() / 2;
    if v.len() % 2 == 1 {
        v[m]
    } else {
        (v[m - 1] + v[m]) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn noise(n: usize, rms_dbfs: f64, mut seed: u64) -> Vec<f32> {
        let amp = 10f64.powf(rms_dbfs / 20.0) * 3f64.sqrt();
        (0..n)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (amp * (2.0 * ((seed >> 11) as f64 / (1u64 << 53) as f64) - 1.0)) as f32
            })
            .collect()
    }

    /// Clap-like burst: decaying noise, 30 ms, starting at `t_ms`.
    fn add_clap(x: &mut [f32], t_ms: f64, peak: f32, seed: u64) {
        let start = (t_ms * SR as f64 / 1000.0) as usize;
        let burst = noise(SR as usize * 30 / 1000, 0.0, seed);
        for (k, b) in burst.iter().enumerate() {
            if let Some(s) = x.get_mut(start + k) {
                let decay = (-(k as f32) / (SR as f32 * 0.006)).exp();
                *s += peak * 0.5 * b * decay;
            }
        }
    }

    fn clicks(n: usize) -> Vec<f64> {
        (0..n).map(|i| 500.0 + i as f64 * 750.0).collect()
    }

    fn room(len_ms: usize) -> Vec<f32> {
        noise(len_ms * SR as usize / 1000, -60.0, 7)
    }

    #[test]
    fn constant_delay_is_measured_to_the_millisecond() {
        let c = clicks(8);
        let mut x = room(7000);
        for (i, &t) in c.iter().enumerate() {
            add_clap(&mut x, t + 87.0, 0.5, i as u64 + 1);
        }
        let r = latency_from(&detect_onsets(&x, SR, -60.0), &c);
        assert_eq!((r.detected, r.expected), (8, 8));
        assert!((r.offset_ms - 87.0).abs() <= 1.0, "{r:?}");
        assert!(r.jitter_ms <= 1.0, "{r:?}");
    }

    #[test]
    fn human_claps_report_their_jitter() {
        let c = clicks(8);
        let mut x = room(7000);
        for (i, &t) in c.iter().enumerate() {
            let d = if i % 2 == 0 { 140.0 } else { 160.0 };
            add_clap(&mut x, t + d, 0.3, i as u64 + 1);
        }
        let r = latency_from(&detect_onsets(&x, SR, -60.0), &c);
        assert_eq!(r.detected, 8);
        assert!((r.offset_ms - 150.0).abs() <= 1.5, "{r:?}");
        assert!((r.jitter_ms - 10.0).abs() <= 1.5, "{r:?}");
    }

    #[test]
    fn missed_claps_lower_detected_and_volume_does_not_matter() {
        let c = clicks(8);
        let mut x = room(7000);
        for (i, &t) in c.iter().enumerate().filter(|(i, _)| *i != 2 && *i != 5) {
            add_clap(&mut x, t + 120.0, 0.05, i as u64 + 1);
        }
        let r = latency_from(&detect_onsets(&x, SR, -60.0), &c);
        assert_eq!(r.detected, 6);
        assert!((r.offset_ms - 120.0).abs() <= 1.0, "{r:?}");
    }

    #[test]
    fn steady_sound_and_room_noise_are_not_transients() {
        assert!(detect_onsets(&room(3000), SR, -60.0).is_empty());
        // 200 ms of silence, then a steady 2 s tone: only its start is a transient.
        let tone: Vec<f32> = (0..SR as usize * 2)
            .map(|i| {
                if i < SR as usize / 5 {
                    0.0
                } else {
                    0.3 * (i as f32 * 440.0 * std::f32::consts::TAU / SR as f32).sin()
                }
            })
            .collect();
        let on = detect_onsets(&tone, SR, -60.0);
        assert_eq!(on.len(), 1, "{on:?}");
        assert!((on[0] - 200.0).abs() < 1.0, "{on:?}");
        let r = latency_from(&[], &clicks(8));
        assert_eq!((r.detected, r.offset_ms, r.jitter_ms), (0, 0.0, 0.0));
    }

    #[test]
    fn early_claps_within_100ms_count_as_negative_offset() {
        let c = clicks(4);
        let mut x = room(4000);
        for (i, &t) in c.iter().enumerate() {
            add_clap(&mut x, t - 40.0, 0.4, i as u64 + 1);
        }
        let r = latency_from(&detect_onsets(&x, SR, -60.0), &c);
        assert_eq!(r.detected, 4);
        assert!((r.offset_ms + 40.0).abs() <= 1.0, "{r:?}");
    }
}
