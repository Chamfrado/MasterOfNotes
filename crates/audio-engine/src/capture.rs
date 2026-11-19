//! Live microphone capture (SPEC-004 R1, R8): cpal (WASAPI on Windows) → lock-free
//! SPSC ring buffer → analysis thread → `on_frame`. The audio callback only
//! downmixes to mono and pushes into the ring: no allocation, no locks, no DSP.

use crate::{AnalysisFrame, Analyzer, AudioError, CaptureConfig};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Ring capacity in seconds of mono audio: the analysis thread may lag this much.
const RING_SECONDS: usize = 2;
const IDLE_SLEEP: Duration = Duration::from_millis(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDevice {
    /// cpal 0.15 has no stable device id; the device name is used.
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub sample_rate: u32,
    pub channels: u16,
}

fn device_err(e: impl std::fmt::Display) -> AudioError {
    AudioError::Device(e.to_string())
}

/// Input devices of the default host (WASAPI on Windows). Devices that report
/// no default input configuration are unusable and are skipped.
pub fn list_input_devices() -> Result<Vec<InputDevice>, AudioError> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    Ok(host
        .input_devices()
        .map_err(device_err)?
        .filter_map(|d| {
            let name = d.name().ok()?;
            let cfg = d.default_input_config().ok()?;
            Some(InputDevice {
                id: name.clone(),
                is_default: default.as_deref() == Some(name.as_str()),
                name,
                sample_rate: cfg.sample_rate().0,
                channels: cfg.channels(),
            })
        })
        .collect())
}

/// Running capture. Analysis runs on its own thread (which also owns the cpal
/// stream, so `Capture` is `Send`). Drop = stop.
pub struct Capture {
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
    sample_rate: u32,
}

impl Capture {
    /// `device_id = None` uses the system default input.
    pub fn start(
        device_id: Option<&str>,
        cfg: CaptureConfig,
        on_frame: impl FnMut(AnalysisFrame) + Send + 'static,
    ) -> Result<Capture, AudioError> {
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let id = device_id.map(str::to_owned);
        let (stop2, error2) = (stop.clone(), error.clone());
        let thread = thread::Builder::new()
            .name("mic-analysis".into())
            .spawn(move || run(id, cfg, on_frame, stop2, error2, ready_tx))
            .map_err(device_err)?;
        match ready_rx.recv() {
            Ok(Ok(sample_rate)) => Ok(Capture {
                stop,
                error,
                thread: Some(thread),
                sample_rate,
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(AudioError::Device(
                    "a thread de análise terminou inesperadamente".into(),
                ))
            }
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Stream error reported by the driver (e.g. device unplugged), if any.
    /// After an error the capture stops producing frames.
    pub fn take_error(&self) -> Option<AudioError> {
        self.error.lock().ok()?.take().map(AudioError::Device)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    device_id: Option<String>,
    cfg: CaptureConfig,
    mut on_frame: impl FnMut(AnalysisFrame),
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    ready: mpsc::SyncSender<Result<u32, AudioError>>,
) {
    let (stream, mut consumer, sample_rate) = match open(device_id.as_deref(), error.clone()) {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    if let Err(e) = stream.play() {
        let _ = ready.send(Err(device_err(e)));
        return;
    }
    let _ = ready.send(Ok(sample_rate));

    let mut analyzer = Analyzer::new(sample_rate, cfg);
    while !stop.load(Ordering::Relaxed) {
        if error.lock().map(|e| e.is_some()).unwrap_or(true) {
            break;
        }
        let n = consumer.slots();
        if n == 0 {
            thread::sleep(IDLE_SLEEP);
            continue;
        }
        if let Ok(chunk) = consumer.read_chunk(n) {
            let (a, b) = chunk.as_slices();
            analyzer.push(a, &mut on_frame);
            analyzer.push(b, &mut on_frame);
            chunk.commit_all();
        }
    }
    drop(stream);
}

type Opened = (cpal::Stream, rtrb::Consumer<f32>, u32);

fn open(device_id: Option<&str>, error: Arc<Mutex<Option<String>>>) -> Result<Opened, AudioError> {
    let host = cpal::default_host();
    let device = match device_id {
        None => host.default_input_device(),
        Some(id) => host
            .input_devices()
            .map_err(device_err)?
            .find(|d| d.name().is_ok_and(|n| n == id)),
    }
    .ok_or_else(|| AudioError::Device("microfone não encontrado".into()))?;
    let supported = device.default_input_config().map_err(device_err)?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.config();
    let (producer, consumer) = rtrb::RingBuffer::new(sample_rate as usize * RING_SECONDS);
    let on_error = move |e: cpal::StreamError| {
        if let Ok(mut slot) = error.lock() {
            slot.get_or_insert(e.to_string());
        }
    };
    use cpal::SampleFormat as F;
    let stream = match supported.sample_format() {
        F::F32 => build::<f32>(&device, &config, channels, producer, on_error),
        F::I16 => build::<i16>(&device, &config, channels, producer, on_error),
        F::U16 => build::<u16>(&device, &config, channels, producer, on_error),
        F::I32 => build::<i32>(&device, &config, channels, producer, on_error),
        other => {
            return Err(AudioError::Device(format!(
                "formato de amostra não suportado: {other}"
            )));
        }
    }?;
    Ok((stream, consumer, sample_rate))
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = channels.max(1);
    let scale = 1.0 / channels as f32;
    device
        .build_input_stream(
            config,
            // Real-time callback: downmix + push only. A full ring drops samples
            // (analysis lagging > RING_SECONDS) instead of blocking the driver.
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                for frame in data.chunks_exact(channels) {
                    let mono = frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() * scale;
                    let _ = producer.push(mono);
                }
            },
            on_error,
            None,
        )
        .map_err(device_err)
}
