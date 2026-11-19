//! Real-hardware checks. Ignored by default (CI has no microphone):
//! `cargo test -p audio-engine --test device -- --ignored --nocapture --test-threads=1`
//! They prove the capture path runs (frames flow at ~100/s with monotonic
//! sample indices). They do NOT validate pitch with a real voice — that is the
//! manual checklist in docs/testing/evidence/.

use audio_engine::{Capture, CaptureConfig, calibrate_noise, hop_len, list_input_devices};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
#[ignore = "needs an audio input device"]
fn lists_input_devices() {
    let devices = list_input_devices().expect("host enumerates devices");
    for d in &devices {
        eprintln!("{d:?}");
    }
    assert!(devices.iter().filter(|d| d.is_default).count() <= 1);
}

#[test]
#[ignore = "needs an audio input device"]
fn captures_frames_from_the_default_device() {
    let frames = Arc::new(Mutex::new(Vec::new()));
    let sink = frames.clone();
    let cap = Capture::start(None, CaptureConfig::default(), move |f| {
        sink.lock().unwrap().push(f)
    })
    .expect("default input opens");
    std::thread::sleep(Duration::from_secs(2));
    let err = cap.take_error();
    let sr = cap.sample_rate();
    let captured = cap.captured_samples();
    drop(cap);
    let frames = frames.lock().unwrap();
    let cal = calibrate_noise(&frames);
    eprintln!(
        "sample rate {sr}, {} frames in 2 s, error {err:?}, room {cal:?}",
        frames.len()
    );
    assert!(err.is_none());
    // Stream start-up latency varies (≈ 0.1–0.6 s measured), so do not bound the
    // count by wall-clock time: require ≥ 1 s of audio, gap-free.
    assert!(frames.len() >= 90, "{} frames", frames.len());
    let hop = hop_len(sr) as u64;
    assert!(
        frames
            .windows(2)
            .all(|w| w[1].sample_index == w[0].sample_index + hop)
    );
    // The callback-side counter is never behind what the analysis has seen.
    let last = frames.last().map_or(0, |f| f.sample_index);
    assert!(captured >= last, "captured {captured} < last frame {last}");
    eprintln!("captured {captured} samples, last frame starts at {last}");
}

#[test]
#[ignore = "needs an audio input device"]
fn unknown_device_is_a_clear_error() {
    let err = Capture::start(
        Some("no such device 1234"),
        CaptureConfig::default(),
        |_| {},
    )
    .err();
    assert!(err.is_some_and(|e| e.to_string().contains("não encontrado")));
}
