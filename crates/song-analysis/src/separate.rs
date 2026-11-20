//! MDX-Net vocal separation (UVR-MDX-NET Voc_FT) through ONNX Runtime.
//!
//! Same pre/post-processing as UVR's MDX demix, measured in the spike
//! (docs/research/auto-reference-spike.md): STFT n_fft 7680, hop 1024, periodic
//! Hann, centred with reflect padding; the model sees 4 × 3072 bins × 256 frames
//! (re/im of two channels). The song is cut into chunks of 255 hops that overlap
//! by n_fft/2 on each side; the overlap is discarded (its frames saw the chunk
//! edge). Vocals are scaled by the model's compensation (1.021).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use crate::{AnalysisError, KnownModel};

pub(crate) const N_FFT: usize = 7680;
pub(crate) const HOP: usize = 1024;
const DIM_F: usize = 3072;
const DIM_T: usize = 256;
const COMPENSATE: f32 = 1.021;
/// Samples per chunk (DIM_T frames, centred).
pub(crate) const CHUNK: usize = HOP * (DIM_T - 1);
const TRIM: usize = N_FFT / 2;
/// New samples per chunk.
pub(crate) const STEP: usize = CHUNK - 2 * TRIM;

/// A loaded separation model.
pub struct Separator {
    session: Session,
    stft: Stft,
    sample_rate: u32,
}

impl Separator {
    /// Loads `model_path` (already checked against `model`'s SHA-256 at install).
    pub fn load(model_path: &Path, model: &KnownModel) -> Result<Self, AnalysisError> {
        if !crate::known_models().iter().any(|k| k.id == model.id) {
            return Err(AnalysisError::Model(format!(
                "\"{}\" não é um modelo conhecido.",
                model.name
            )));
        }
        fn model_err(e: impl std::fmt::Display) -> AnalysisError {
            AnalysisError::Model(format!("não foi possível abrir o modelo ({e})."))
        }
        // The default arena keeps ~2.5 GB; without it (and without memory
        // patterns) the peak is ~1.2 GB at the same speed
        // (docs/testing/auto-reference.md).
        let session = Session::builder(environment()?)
            .map_err(model_err)?
            .with_memory_pattern(false)
            .map_err(model_err)?
            .with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()])
            .map_err(model_err)?
            .commit_from_file(model_path)
            .map_err(model_err)?;
        Ok(Self {
            session,
            stft: Stft::new(),
            sample_rate: model.sample_rate,
        })
    }

    /// Separated voice, mono, same rate as the input (which must be the model's
    /// rate, 44.1 kHz). `progress` goes 0..1 per chunk (~6 s of audio);
    /// `cancel` is checked between chunks (Err(Cancelled)).
    pub fn vocals(
        &mut self,
        mono: &[f32],
        sample_rate: u32,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<f32>, AnalysisError> {
        if sample_rate != self.sample_rate {
            return Err(AnalysisError::Input(format!(
                "O áudio precisa estar em {} Hz (veio em {sample_rate} Hz).",
                self.sample_rate
            )));
        }
        let n = mono.len();
        let chunks = n.div_ceil(STEP);
        // [TRIM zeros | song | zeros up to the last chunk's end]
        let mut padded = vec![0.0f32; TRIM + chunks * STEP + TRIM];
        padded[TRIM..TRIM + n].copy_from_slice(mono);
        let mut out = Vec::with_capacity(chunks * STEP);
        progress(0.0);
        for k in 0..chunks {
            if cancel.load(Ordering::Relaxed) {
                return Err(AnalysisError::Cancelled);
            }
            let seg = &padded[k * STEP..k * STEP + CHUNK];
            let input = self.stft.forward(seg);
            let x = Tensor::from_array(([1usize, 4, DIM_F, DIM_T], input))
                .map_err(|e| AnalysisError::Model(e.to_string()))?;
            let outputs = self
                .session
                .run(ort::inputs![x])
                .map_err(|e| AnalysisError::Model(format!("erro ao separar a voz ({e}).")))?;
            let (_, pred) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| AnalysisError::Model(e.to_string()))?;
            if pred.len() != 4 * DIM_F * DIM_T {
                return Err(AnalysisError::Model(
                    "o modelo devolveu um formato inesperado.".into(),
                ));
            }
            let y = self.stft.inverse(pred);
            out.extend(y[TRIM..TRIM + STEP].iter().map(|s| s * COMPENSATE));
            progress((k + 1) as f32 / chunks as f32);
        }
        out.truncate(n);
        Ok(out)
    }
}

/// The ONNX Runtime environment, loaded once per process.
fn environment() -> Result<&'static Environment, AnalysisError> {
    static ENV: OnceLock<Result<Environment, String>> = OnceLock::new();
    ENV.get_or_init(|| {
        let path = dylib_path();
        if !path.is_file() {
            return Err(format!(
                "o componente ONNX Runtime não foi encontrado ({}). Reinstale o Master Of Notes.",
                path.display()
            ));
        }
        ort::init_from(&path)
            .map_err(|e| e.to_string())?
            .build()
            .map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(|e| AnalysisError::Model(e.clone()))
}

/// `ORT_DYLIB_PATH` (set by the app from its resource dir), else next to the exe.
/// Always an absolute path: never the older copy Windows keeps in System32.
fn dylib_path() -> PathBuf {
    let name = format!(
        "{}onnxruntime{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    std::env::var_os("ORT_DYLIB_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(|d| d.join(&name)))
        })
        .unwrap_or_else(|| PathBuf::from(name))
}

/// STFT/iSTFT of one chunk, matching librosa (centre, reflect pad, periodic Hann).
pub(crate) struct Stft {
    fwd: std::sync::Arc<dyn Fft<f32>>,
    inv: std::sync::Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    /// Σ window² over the chunk's frames (iSTFT normalisation), length CHUNK + N_FFT.
    wss: Vec<f32>,
    buf: Vec<Complex32>,
}

impl Stft {
    pub(crate) fn new() -> Self {
        let mut planner = FftPlanner::new();
        let window: Vec<f32> = (0..N_FFT)
            .map(|i| {
                let x = std::f64::consts::TAU * i as f64 / N_FFT as f64;
                (0.5 - 0.5 * x.cos()) as f32
            })
            .collect();
        let mut wss = vec![0.0f32; CHUNK + N_FFT];
        for t in 0..DIM_T {
            for (i, w) in window.iter().enumerate() {
                wss[t * HOP + i] += w * w;
            }
        }
        Self {
            fwd: planner.plan_fft_forward(N_FFT),
            inv: planner.plan_fft_inverse(N_FFT),
            window,
            wss,
            buf: vec![Complex32::default(); N_FFT],
        }
    }

    /// `seg` (CHUNK samples, mono) → model input [4, DIM_F, DIM_T]; both
    /// stereo channels are the mono signal.
    pub(crate) fn forward(&mut self, seg: &[f32]) -> Vec<f32> {
        debug_assert_eq!(seg.len(), CHUNK);
        let half = N_FFT / 2;
        // Reflect padding (numpy "reflect": the edge sample is not repeated).
        let at = |j: isize| -> f32 {
            let n = seg.len() as isize;
            let i = if j < 0 {
                -j
            } else if j >= n {
                2 * (n - 1) - j
            } else {
                j
            };
            seg[i as usize]
        };
        let mut out = vec![0.0f32; 4 * DIM_F * DIM_T];
        for t in 0..DIM_T {
            let start = (t * HOP) as isize - half as isize;
            for (i, c) in self.buf.iter_mut().enumerate() {
                *c = Complex32::new(at(start + i as isize) * self.window[i], 0.0);
            }
            self.fwd.process(&mut self.buf);
            for f in 0..DIM_F {
                let c = self.buf[f];
                for ch in 0..2 {
                    out[((2 * ch) * DIM_F + f) * DIM_T + t] = c.re;
                    out[((2 * ch + 1) * DIM_F + f) * DIM_T + t] = c.im;
                }
            }
        }
        out
    }

    /// Model output [4, DIM_F, DIM_T] → CHUNK samples, mean of the two channels.
    pub(crate) fn inverse(&mut self, spec: &[f32]) -> Vec<f32> {
        let half = N_FFT / 2;
        let mut y = vec![0.0f32; CHUNK + N_FFT];
        let get = |c: usize, f: usize, t: usize| spec[(c * DIM_F + f) * DIM_T + t];
        for t in 0..DIM_T {
            self.buf.fill(Complex32::default());
            for f in 0..DIM_F {
                let re = 0.5 * (get(0, f, t) + get(2, f, t));
                let im = 0.5 * (get(1, f, t) + get(3, f, t));
                self.buf[f] = Complex32::new(re, im);
                if f > 0 {
                    self.buf[N_FFT - f] = Complex32::new(re, -im);
                }
            }
            // irfft ignores the imaginary part of DC.
            self.buf[0].im = 0.0;
            self.inv.process(&mut self.buf);
            for (i, c) in self.buf.iter().enumerate() {
                y[t * HOP + i] += c.re / N_FFT as f32 * self.window[i];
            }
        }
        (0..CHUNK)
            .map(|i| {
                let w = self.wss[i + half];
                if w > 1e-8 { y[i + half] / w } else { 0.0 }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stft_then_istft_returns_the_chunk_below_the_cut_off() {
        // Content below bin DIM_F (17.6 kHz) survives the band limit exactly.
        let seg: Vec<f32> = (0..CHUNK)
            .map(|i| {
                let t = i as f64 / 44_100.0;
                (0.4 * (std::f64::consts::TAU * 220.0 * t).sin()
                    + 0.2 * (std::f64::consts::TAU * 3_520.0 * t + 1.0).sin())
                    as f32
            })
            .collect();
        let mut s = Stft::new();
        let spec = s.forward(&seg);
        let back = s.inverse(&spec);
        // Only the kept part: the trimmed edges saw the reflect padding.
        let err = seg[TRIM..TRIM + STEP]
            .iter()
            .zip(&back[TRIM..TRIM + STEP])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-3, "max error {err}");
    }

    #[test]
    fn chunks_tile_the_song_without_gaps() {
        // Chunk k covers padded[k*STEP .. k*STEP+CHUNK]; its kept part is the
        // song samples [k*STEP, (k+1)*STEP).
        assert_eq!(CHUNK - 2 * TRIM, STEP);
        assert_eq!(STEP, 253_440);
        for n in [1usize, STEP - 1, STEP, STEP + 1, 10_584_000] {
            let chunks = n.div_ceil(STEP);
            assert!(chunks * STEP >= n && (chunks - 1) * STEP < n);
        }
    }

    #[test]
    fn missing_runtime_is_a_model_error_in_portuguese() {
        // In the test binary there is no onnxruntime next to the exe (and
        // ORT_DYLIB_PATH is unset in CI), so loading must fail cleanly.
        if std::env::var_os("ORT_DYLIB_PATH").is_some() {
            return;
        }
        let m = crate::known_models()[0];
        match Separator::load(Path::new("nao-existe.onnx"), &m) {
            Err(AnalysisError::Model(msg)) => assert!(msg.contains("ONNX Runtime"), "{msg}"),
            Err(e) => panic!("{e}"),
            Ok(_) => panic!("loaded without a runtime"),
        }
    }

    #[test]
    fn separator_and_errors_can_move_to_a_job_thread() {
        fn send<T: Send>() {}
        send::<Separator>();
        send::<AnalysisError>();
    }

    #[test]
    fn unknown_model_is_refused() {
        let mut m = crate::known_models()[0];
        m.id = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(matches!(
            Separator::load(Path::new("x.onnx"), &m),
            Err(AnalysisError::Model(_))
        ));
    }
}
