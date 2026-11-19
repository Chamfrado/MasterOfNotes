//! Microphone commands (SPEC-004, T-004-04): device list, live meter, noise calibration.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use audio_engine::{AnalysisFrame, Capture, CaptureConfig, InputDevice, NoiseCalibration};
use serde::Serialize;
use serde_json::json;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::commands::{AppState, blocking};
use crate::error::{AppError, AppResult};
use crate::library::lock;
use crate::settings;

/// Mirrors `MicFrame` in ipc.ts.
// TODO(T-006-05): replace with crate::performance::MicFrame once Agent 2 delivers it.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicFrame {
    pub rms_dbfs: f32,
    pub f0_hz: Option<f32>,
    pub confidence: f32,
    pub voiced: bool,
    pub clipped: bool,
}

impl From<&AnalysisFrame> for MicFrame {
    fn from(f: &AnalysisFrame) -> Self {
        Self {
            rms_dbfs: f.rms_dbfs,
            f0_hz: f.f0_hz,
            confidence: f.confidence,
            voiced: f.voiced,
            clipped: f.clipped,
        }
    }
}

/// The running meter, if any. Drop = stop capturing.
#[derive(Default)]
pub struct MicMonitor(Mutex<Option<Capture>>);

impl MicMonitor {
    /// Stops the meter (also used before a performance takes the microphone).
    pub fn stop(&self) {
        let capture = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(c) = capture
            && let Some(e) = c.take_error()
        {
            log::warn!("mic monitor stopped after device error: {e}");
        }
    }
}

/// Frames arrive every 10 ms; the meter gets every 3rd (~33 Hz), with clipping
/// OR-ed over the skipped frames so short clips are not hidden.
const METER_EVERY: u32 = 3;

pub fn audio_err(e: audio_engine::AudioError) -> AppError {
    log::error!("audio: {e}");
    AppError::new(
        "audio_device",
        format!(
            "Problema com o microfone: {e}. Verifique se ele está conectado e liberado nas configurações de privacidade do Windows."
        ),
    )
}

fn capture_config(app: &AppHandle) -> AppResult<CaptureConfig> {
    let s = settings::load(&lock(app.state::<AppState>().db()?))?;
    let mut cfg = CaptureConfig::default();
    if let Some(n) = s.noise_floor_dbfs {
        cfg.noise_floor_dbfs = n as f32;
    }
    Ok(cfg)
}

#[tauri::command]
pub async fn list_input_devices() -> AppResult<Vec<InputDevice>> {
    blocking(|| audio_engine::list_input_devices().map_err(audio_err)).await
}

#[tauri::command]
pub async fn start_mic_monitor(
    app: AppHandle,
    device_id: Option<String>,
    on_frame: Channel<MicFrame>,
) -> AppResult<()> {
    blocking(move || {
        let monitor = app.state::<MicMonitor>();
        monitor.stop();
        let cfg = capture_config(&app)?;
        let mut n = 0u32;
        let mut clipped = false;
        let capture = Capture::start(device_id.as_deref(), cfg, move |f| {
            n += 1;
            clipped |= f.clipped;
            if n.is_multiple_of(METER_EVERY) {
                let mut frame = MicFrame::from(&f);
                frame.clipped = clipped;
                clipped = false;
                // The UI may have gone away; the meter keeps running until stop.
                let _ = on_frame.send(frame);
            }
        })
        .map_err(audio_err)?;
        *monitor
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(capture);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn stop_mic_monitor(monitor: State<'_, MicMonitor>) -> AppResult<()> {
    monitor.stop();
    Ok(())
}

/// Records `seconds` of room noise (user silent) and saves `noiseFloorDbfs`.
#[tauri::command]
pub async fn calibrate_noise(
    app: AppHandle,
    device_id: Option<String>,
    seconds: f64,
) -> AppResult<NoiseCalibration> {
    if !(1.0..=10.0).contains(&seconds) {
        return Err(AppError::invalid_input(
            "A calibração deve durar entre 1 e 10 segundos.",
        ));
    }
    blocking(move || {
        let frames = Arc::new(Mutex::new(Vec::<AnalysisFrame>::new()));
        let sink = frames.clone();
        let capture = Capture::start(
            device_id.as_deref(),
            CaptureConfig::default(),
            move |f| {
                if let Ok(mut v) = sink.lock() {
                    v.push(f);
                }
            },
        )
        .map_err(audio_err)?;
        std::thread::sleep(Duration::from_secs_f64(seconds));
        let device_error = capture.take_error();
        drop(capture);
        if let Some(e) = device_error {
            return Err(audio_err(e));
        }
        let frames = std::mem::take(&mut *frames.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
        if frames.is_empty() {
            return Err(AppError::new(
                "audio_device",
                "O microfone não enviou áudio durante a calibração. Verifique a conexão e tente de novo.",
            ));
        }
        let result = audio_engine::calibrate_noise(&frames);
        let state = app.state::<AppState>();
        settings::write(
            &mut lock(state.db()?),
            json!({ "noiseFloorDbfs": result.noise_floor_dbfs })
                .as_object()
                .cloned()
                .unwrap_or_default(),
        )?;
        Ok(result)
    })
    .await
}
