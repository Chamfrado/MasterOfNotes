//! Measurements with the real separation model (SPEC-012 R7). Ignored in CI:
//! the model and ONNX Runtime are never in the repo.
//!
//! ```sh
//! # model: UVR-MDX-NET-Voc_FT.onnx (see known_models()), DLL: onnxruntime.dll 1.31.1
//! # material: raw f32 LE mono 44.1 kHz + truth JSON, made by the spike generator
//! #           (docs/testing/auto-reference.md, "Como reproduzir")
//! SA_MODEL=.../UVR-MDX-NET-Voc_FT.onnx SA_MATERIAL=.../material ORT_DYLIB_PATH=.../onnxruntime.dll \
//!   cargo test -p song-analysis --release --test real_model -- --ignored --nocapture --test-threads 1
//! ```

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use scoring_engine::{
    Difficulty, MelodyReference, Note, ScoreResult, Scorer, VoiceFrame, parse_reference_json,
};
use song_analysis::{
    AnalysisError, NO_VOICE_DB, Separator, align_offset_ms, known_models, sha256_file, transcribe,
    voice_level_db,
};

const SR: u32 = 44_100;

fn env_path(k: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(k).unwrap_or_else(|| panic!("set {k}")))
}

fn load(name: &str) -> Vec<f32> {
    let b = std::fs::read(env_path("SA_MATERIAL").join(name)).unwrap();
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

fn truth() -> Vec<Note> {
    let json = std::fs::read_to_string(env_path("SA_MATERIAL").join("pop_truth.json")).unwrap();
    parse_reference_json(&json).unwrap().notes
}

fn separator() -> Separator {
    let path = env_path("SA_MODEL");
    let m = known_models()[0];
    assert_eq!(sha256_file(&path).unwrap(), m.id, "not the known model");
    Separator::load(&path, &m).unwrap()
}

/// Spike metric: one-to-one, onset within ±100 ms, pitch within ±50 c.
fn note_match(truth: &[Note], got: &[Note]) -> usize {
    let mut used = vec![false; got.len()];
    let mut correct = 0;
    for t in truth {
        let best = got
            .iter()
            .enumerate()
            .filter(|(j, g)| {
                !used[*j]
                    && (g.start_ms as f64 - t.start_ms as f64).abs() <= 100.0
                    && (g.midi - t.midi).abs() <= 0.5
            })
            .min_by_key(|(_, g)| (g.start_ms as i64 - t.start_ms as i64).abs())
            .map(|(j, _)| j);
        if let Some(j) = best {
            used[j] = true;
            correct += 1;
        }
    }
    correct
}

/// A singer who sings the TRUE melody perfectly, scored against `generated`.
fn perfect_singer_against(truth: &[Note], generated: Vec<Note>) -> ScoreResult {
    let r = MelodyReference {
        format_version: 1,
        notes: generated,
    };
    let mut s = Scorer::new(&r, 0, Difficulty::Normal);
    let end = truth
        .iter()
        .map(|n| n.start_ms + n.dur_ms)
        .max()
        .unwrap_or(0) as f64;
    let mut t = 0.0;
    while t < end {
        let m = truth
            .iter()
            .find(|n| t >= n.start_ms as f64 && t < (n.start_ms + n.dur_ms) as f64);
        s.push(VoiceFrame {
            t_ms: t,
            f0_hz: m.map(|n| (440.0 * 2f64.powf((n.midi as f64 - 69.0) / 12.0)) as f32),
            confidence: 0.95,
            voiced: m.is_some(),
        });
        t += 10.0;
    }
    s.finish()
}

#[test]
#[ignore = "needs the real model and onnxruntime (see the module doc)"]
fn r7_separation_transcription_and_no_voice() {
    let mut sep = separator();
    let truth = truth();
    let cancel = AtomicBool::new(false);
    eprintln!(
        "| Material | Relógio separação | Voz ÷ mistura | Notas | Corretas / recall | Precisão | Cantor perfeito |"
    );
    for (file, has_voice) in [
        ("pop_guide.f32", true),
        ("pop_lead.f32", false),
        ("pop_karaoke.f32", false),
    ] {
        let mix = load(file);
        let t = Instant::now();
        let vocals = sep.vocals(&mix, SR, &mut |_| {}, &cancel).unwrap();
        let sep_s = t.elapsed().as_secs_f64();
        assert_eq!(vocals.len(), mix.len());
        let level = voice_level_db(&vocals, &mix);
        let t = Instant::now();
        let notes = transcribe(&vocals, SR);
        let tr_s = t.elapsed().as_secs_f64();
        let correct = note_match(&truth, &notes);
        let score = perfect_singer_against(&truth, notes.clone()).score;
        eprintln!(
            "| {file} ({:.0} s) | {sep_s:.0} s (+{tr_s:.1} s transcrição) | {level:+.1} dB | {} | {correct} / {:.1}% | {:.1}% | {score:.1} |",
            mix.len() as f64 / SR as f64,
            notes.len(),
            100.0 * correct as f64 / truth.len() as f64,
            100.0 * correct as f64 / notes.len().max(1) as f64,
        );
        assert_eq!(level >= NO_VOICE_DB, has_voice, "{file}: {level} dB");
        if has_voice {
            assert!(correct as f64 >= 0.95 * notes.len() as f64, "precision");
        }
    }
}

#[test]
#[ignore = "needs the real model and onnxruntime (see the module doc)"]
fn r7_cancel_stops_within_one_chunk() {
    let mut sep = separator();
    let mix = load("pop_guide.f32");
    let cancel = AtomicBool::new(false);
    let mut asked = None;
    let r = sep.vocals(
        &mix,
        SR,
        &mut |p| {
            if p > 0.0 && asked.is_none() {
                cancel.store(true, Ordering::Relaxed);
                asked = Some(Instant::now());
            }
        },
        &cancel,
    );
    let after = asked.map(|t| t.elapsed().as_secs_f64());
    eprintln!("cancelado; retorno {after:?} s depois do pedido");
    assert!(matches!(r, Err(AnalysisError::Cancelled)));
}

#[test]
#[ignore = "needs the synthetic material only (no model)"]
fn r5_alignment_on_the_four_minute_song() {
    let truth = truth();
    eprintln!("| Áudio | MIDI deslocado | Proposta | Erro | Confiança | Tempo |");
    for file in ["pop_guide.f32", "pop_lead.f32", "pop_karaoke.f32"] {
        let mix = load(file);
        for shift in [-1200i64, 1200, -4321] {
            let notes: Vec<Note> = truth
                .iter()
                .filter_map(|n| {
                    let s = n.start_ms as i64 + shift;
                    (s >= 0).then(|| Note {
                        start_ms: s as u32,
                        ..n.clone()
                    })
                })
                .collect();
            let t = Instant::now();
            let r = align_offset_ms(&notes, &mix, SR);
            eprintln!(
                "| {file} | {shift:+} ms | {:+} ms | {} ms | {:.2} | {:.1} s |",
                r.offset_ms,
                (r.offset_ms as i64 + shift).abs(),
                r.confidence,
                t.elapsed().as_secs_f64()
            );
        }
    }
}
