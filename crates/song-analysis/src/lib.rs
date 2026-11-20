//! Automatic reference and alignment (SPEC-012, ADR-0004 front B).
//!
//! - [`Separator`]: vocal separation with a user-supplied, known ONNX model
//!   (ONNX Runtime loaded at run time from `ORT_DYLIB_PATH`, else next to the exe).
//! - [`voice_level_db`] / [`NO_VOICE_DB`]: "this video has no sung melody".
//! - [`transcribe`]: our YIN (`audio_engine::analyze_buffer`) + `scoring_engine::segment_notes`.
//! - [`align_offset_ms`]: offset between reference notes and the video audio (chroma).
//!
//! No network I/O; the model is never downloaded, bundled or sent anywhere.
//! Evidence for every constant: docs/testing/auto-reference.md.

mod align;
mod separate;

use std::io::Read;
use std::path::Path;

use audio_engine::{CaptureConfig, analyze_buffer, window_len};
use scoring_engine::{Note, VoiceFrame, segment_notes};
use sha2::{Digest, Sha256};

pub use align::{AlignResult, LOW_CONFIDENCE, align_offset_ms};
pub use separate::Separator;

/// A separation model the app accepts (identified by the SHA-256 of the file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownModel {
    /// SHA-256 of the `.onnx` file, lowercase hex.
    pub id: &'static str,
    pub name: &'static str,
    pub file_name: &'static str,
    /// Exact file size (checked before hashing).
    pub size_bytes: u64,
    pub source_url: &'static str,
    /// License summary in pt-BR (shown to the user).
    pub license: &'static str,
    pub sample_rate: u32,
}

const MODELS: &[KnownModel] = &[KnownModel {
    id: "534b2070fcc7df514b13ef660dc8cbb328679c2374d04354a5c42bb14ecce111",
    name: "UVR-MDX-NET Voc_FT",
    file_name: "UVR-MDX-NET-Voc_FT.onnx",
    size_bytes: 66_762_490,
    source_url: "https://github.com/TRvlvr/model_repo/releases/download/all_public_uvr_models/UVR-MDX-NET-Voc_FT.onnx",
    license: "Modelo do projeto Ultimate Vocal Remover (UVR). Os autores não declararam uma licença para os pesos do modelo; o código do UVR é MIT e os autores pedem crédito. Não sabemos se o uso comercial é permitido. O Master Of Notes não distribui este arquivo: você o baixa da fonte original e o uso é de sua responsabilidade.",
    sample_rate: 44_100,
}];

/// Models the app accepts.
pub fn known_models() -> &'static [KnownModel] {
    MODELS
}

/// SHA-256 of a file, lowercase hex (streamed; call off the UI thread).
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("Falha no modelo de separação: {0}")]
    Model(String),
    #[error("{0}")]
    Input(String),
    #[error("Operação cancelada.")]
    Cancelled,
}

/// Below this, the separated voice is "no voice" (instrumental / karaoke video).
/// Measured: voice present −3 to −6 dB, no voice −37 to −55 dB
/// (docs/testing/auto-reference.md); the threshold sits in the middle of the gap.
pub const NO_VOICE_DB: f32 = -20.0;

/// Level of the separated voice relative to the mix (dB, RMS over the whole
/// song). `-inf` when either is silent.
pub fn voice_level_db(vocals: &[f32], mix: &[f32]) -> f32 {
    let (v, m) = (rms(vocals), rms(mix));
    if v <= 0.0 || m <= 0.0 {
        return f32::NEG_INFINITY;
    }
    (20.0 * (v / m).log10()) as f32
}

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|&s| s as f64 * s as f64).sum::<f64>() / x.len() as f64).sqrt()
}

/// Separation leaves a faint residue where nobody sings; frames more than this
/// below the stem's loud parts (p90 of 10 ms blocks) are not voice.
const STEM_FLOOR_DB: f64 = 30.0;

/// Notes sung in a separated vocal stem, in video time (ms from 0, no offset).
pub fn transcribe(vocals: &[f32], sample_rate: u32) -> Vec<Note> {
    let default = CaptureConfig::default();
    let gate = stem_gate_dbfs(vocals, sample_rate)
        .max((default.noise_floor_dbfs + default.gate_margin_db) as f64);
    let cfg = CaptureConfig {
        noise_floor_dbfs: gate as f32 - default.gate_margin_db,
        gate_margin_db: default.gate_margin_db,
    };
    let centre = window_len(sample_rate) as f64 / 2.0;
    let frames: Vec<VoiceFrame> = analyze_buffer(vocals, sample_rate, &cfg)
        .iter()
        .map(|f| VoiceFrame {
            t_ms: (f.sample_index as f64 + centre) * 1000.0 / sample_rate as f64,
            f0_hz: f.f0_hz,
            confidence: f.confidence,
            voiced: f.voiced,
        })
        .collect();
    segment_notes(&frames)
}

/// p90 of the 10 ms block levels minus [`STEM_FLOOR_DB`] (dBFS).
fn stem_gate_dbfs(x: &[f32], sample_rate: u32) -> f64 {
    let block = (sample_rate as usize / 100).max(1);
    let mut db: Vec<f64> = x
        .chunks(block)
        .map(|c| 20.0 * rms(c).max(1e-10).log10())
        .collect();
    if db.is_empty() {
        return f64::NEG_INFINITY;
    }
    let k = (db.len() * 9 / 10).min(db.len() - 1);
    db.select_nth_unstable_by(k, f64::total_cmp);
    db[k] - STEM_FLOOR_DB
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64, secs: f64, amp: f64, sr: u32) -> Vec<f32> {
        (0..(secs * sr as f64) as usize)
            .map(|i| (amp * (std::f64::consts::TAU * hz * i as f64 / sr as f64).sin()) as f32)
            .collect()
    }

    #[test]
    fn voice_level_is_the_rms_ratio_and_silence_is_minus_infinity() {
        let mix = tone(220.0, 1.0, 0.5, 44_100);
        let half: Vec<f32> = mix.iter().map(|s| s * 0.5).collect();
        assert!((voice_level_db(&half, &mix) + 6.02).abs() < 0.05);
        let quiet: Vec<f32> = mix.iter().map(|s| s * 0.01).collect();
        assert!(voice_level_db(&quiet, &mix) < NO_VOICE_DB);
        assert!(voice_level_db(&half, &mix) > NO_VOICE_DB);
        assert_eq!(voice_level_db(&[0.0; 10], &mix), f32::NEG_INFINITY);
        assert_eq!(voice_level_db(&half, &[]), f32::NEG_INFINITY);
    }

    #[test]
    fn transcribe_ignores_separation_residue_and_finds_the_notes() {
        let sr = 44_100;
        // 0.5 s A3, 0.5 s residue (−40 dB, a different pitch), 0.5 s C4.
        let mut x = tone(220.0, 0.5, 0.3, sr);
        x.extend(tone(330.0, 0.5, 0.003, sr));
        x.extend(tone(261.63, 0.5, 0.3, sr));
        let notes = transcribe(&x, sr);
        let midi: Vec<f32> = notes.iter().map(|n| n.midi).collect();
        assert_eq!(midi, vec![57.0, 60.0], "{notes:?}");
        assert!(
            (notes[1].start_ms as f64 - 1000.0).abs() < 40.0,
            "{notes:?}"
        );
    }

    #[test]
    fn sha256_of_a_known_file() {
        let p = std::env::temp_dir().join(format!("song-analysis-sha-{}", std::process::id()));
        std::fs::write(&p, b"abc").unwrap();
        let h = sha256_file(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn known_models_are_well_formed() {
        for m in known_models() {
            assert_eq!(m.id.len(), 64);
            assert!(
                m.id.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
            assert!(m.source_url.starts_with("https://") && m.size_bytes > 0);
        }
    }
}
