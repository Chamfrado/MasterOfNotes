//! Real-hardware checks. Ignored by default (CI has no microphone):
//! `cargo test -p audio-engine --test device -- --ignored --nocapture --test-threads=1`
//! They prove the capture path runs (frames flow at ~100/s with monotonic
//! sample indices). They do NOT validate pitch with a real voice — that is the
//! manual checklist in docs/testing/evidence/.

use audio_engine::{Capture, CaptureConfig, calibrate_noise, list_input_devices};
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
    drop(cap);
    let frames = frames.lock().unwrap();
    let cal = calibrate_noise(&frames);
    eprintln!(
        "sample rate {sr}, {} frames in 2 s, error {err:?}, room {cal:?}",
        frames.len()
    );
    assert!(err.is_none());
    assert!(
        (150..=210).contains(&frames.len()),
        "{} frames",
        frames.len()
    );
    assert!(
        frames
            .windows(2)
            .all(|w| w[1].sample_index > w[0].sample_index)
    );
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
