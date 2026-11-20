// ARCHIVED COPY (not compiled). Measured scoring_engine::import_ultrastar @ agent-2/work 92fb1ad (T-013-04, findings in ultrastar-qa.md).
// To reproduce: copy to crates/scoring-engine/tests/ultrastar_qa.rs.

//! QA probe (Agent 4, T-013-04): scoring_engine::import_ultrastar (owner: Agent 2) against the self-authored
//! fixtures in tests/fixtures/references/ultrastar/ (expected values in its README). MEASUREMENT ONLY, local copy:
//! copy to crates/scoring-engine/tests/ultrastar_qa.rs and run
//!   cargo test -p scoring-engine --test ultrastar_qa -- --nocapture --test-threads=1
//! Findings: docs/testing/evidence/ultrastar-qa.md

use scoring_engine::import_ultrastar;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/references/ultrastar")
}

/// (start_ms, dur_ms, midi, lyric)
type Exp = &'static [(u32, u32, f32, &'static str)];

const VALID: &[(&str, Exp, bool)] = &[
    ("basic.txt", &[(1000, 200, 60.0, "Lá"), (1200, 200, 62.0, "ri"), (1900, 200, 64.0, "la"), (2100, 100, 65.0, "~")], false),
    ("bpm-comma.txt", &[(0, 399, 60.0, "a"), (797, 399, 62.0, "b")], false),
    ("relative.txt", &[(500, 200, 60.0, "um"), (700, 200, 62.0, "dois"), (1100, 200, 64.0, "três"), (1400, 100, 65.0, "quatro")], false),
    ("duet.txt", &[(0, 200, 60.0, "eu"), (200, 200, 62.0, "vou")], true),
    ("duet-p-space.txt", &[(0, 200, 60.0, "eu")], true),
    ("after-end.txt", &[(0, 200, 60.0, "fim")], false),
    ("cp1252.txt", &[(0, 200, 60.0, "Ação"), (200, 200, 62.0, "pé")], false),
    ("bom.txt", &[(0, 200, 60.0, "oi")], false),
    ("crlf.txt", &[(0, 200, 60.0, "oi"), (200, 200, 62.0, "tchau")], false),
];

/// (file, substring the pt-BR error must contain; "" = any error)
const INVALID: &[(&str, &str)] = &[
    ("invalid-bpm-zero.txt", "BPM"),
    ("invalid-bpm-negative.txt", "BPM"),
    ("invalid-bpm-text.txt", "BPM"),
    ("invalid-bpm-missing.txt", "BPM"),
    ("invalid-truncated-line.txt", "linha 4"),
    ("invalid-pitch-high.txt", "linha 4"),
    ("invalid-pitch-low.txt", "linha 3"),
    ("invalid-negative-length.txt", "linha 3"),
    ("invalid-only-freestyle.txt", ""),
    ("invalid-empty.txt", ""),
    ("invalid-huge-numbers.txt", "linha 3"),
];

const EDGE: &[&str] = &["edge-gap-negative.txt", "edge-overlap.txt", "edge-header-case.txt", "edge-no-syllable.txt"];

fn show(name: &str) -> String {
    let bytes = std::fs::read(dir().join(name)).expect(name);
    match std::panic::catch_unwind(|| import_ultrastar(&bytes)) {
        Err(_) => "PÂNICO".into(),
        Ok(Err(e)) => format!("erro: {e}"),
        Ok(Ok((r, info))) => format!(
            "{} notas {:?} | título {:?} | dueto {}",
            r.notes.len(),
            r.notes.iter().map(|n| (n.start_ms, n.dur_ms, n.midi, n.lyric.clone().unwrap_or_default())).collect::<Vec<_>>(),
            info.title,
            info.duet
        ),
    }
}

#[test]
fn ultrastar_fixtures() {
    let mut problems = Vec::new();
    eprintln!("\n### Válidos");
    for (name, exp, duet) in VALID {
        let bytes = std::fs::read(dir().join(name)).expect(name);
        eprintln!("- {name}: {}", show(name));
        match std::panic::catch_unwind(|| import_ultrastar(&bytes)) {
            Ok(Ok((r, info))) => {
                let got: Vec<_> = r.notes.iter().map(|n| (n.start_ms, n.dur_ms, n.midi, n.lyric.clone().unwrap_or_default())).collect();
                let ok = got.len() == exp.len()
                    && got.iter().zip(exp.iter()).all(|(g, e)| g.0.abs_diff(e.0) <= 1 && g.1.abs_diff(e.1) <= 1 && (g.2 - e.2).abs() < 0.01 && g.3 == e.3);
                if !ok {
                    problems.push(format!("{name}: notas diferentes do esperado {exp:?}"));
                }
                if info.duet != *duet {
                    problems.push(format!("{name}: dueto {} (esperado {duet})", info.duet));
                }
                if got.iter().any(|g| g.3.contains('\r') || g.3.contains('\u{feff}')) {
                    problems.push(format!("{name}: \\r ou BOM na sílaba"));
                }
                if matches!(*name, "cp1252.txt" | "bom.txt") && info.title.as_deref() != Some("Canção de Teste") {
                    problems.push(format!("{name}: título {:?}", info.title));
                }
            }
            Ok(Err(e)) => problems.push(format!("{name}: erro inesperado: {e}")),
            Err(_) => problems.push(format!("{name}: PÂNICO")),
        }
    }
    eprintln!("\n### Inválidos (erro em pt-BR; número da linha quando é uma linha)");
    for (name, needle) in INVALID {
        let out = show(name);
        eprintln!("- {name}: {out}");
        if out == "PÂNICO" || !out.starts_with("erro: ") || !out.contains(needle) {
            problems.push(format!("{name}: esperado erro contendo {needle:?}, veio {out}"));
        }
    }
    eprintln!("\n### Limite (qualquer comportamento documentado; nunca pânico)");
    for name in EDGE {
        let out = show(name);
        eprintln!("- {name}: {out}");
        if out == "PÂNICO" {
            problems.push(format!("{name}: PÂNICO"));
        }
    }
    eprintln!("\n### Problemas\n{}", if problems.is_empty() { "nenhum".to_string() } else { problems.join("\n") });
}

/// Light fuzz: every prefix of every fixture (truncated files) and random byte flips must never panic.
#[test]
fn ultrastar_fuzz_no_panic() {
    let mut panics = Vec::new();
    let mut runs = 0;
    let mut seed = 0x5EEDu64;
    for entry in std::fs::read_dir(dir()).expect("fixtures") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "txt") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read");
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for cut in 0..=bytes.len() {
            runs += 1;
            if std::panic::catch_unwind(|| import_ultrastar(&bytes[..cut])).is_err() {
                panics.push(format!("{name} cortado em {cut} bytes"));
            }
        }
        for _ in 0..300 {
            let mut b = bytes.clone();
            if b.is_empty() {
                break;
            }
            for _ in 0..3 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let i = (seed as usize) % b.len();
                b[i] = (seed >> 32) as u8;
            }
            runs += 1;
            if std::panic::catch_unwind(|| import_ultrastar(&b)).is_err() {
                panics.push(format!("{name} com bytes trocados (semente {seed:#x})"));
            }
        }
    }
    eprintln!("\n### Fuzz leve: {runs} execuções, {} pânicos{}", panics.len(), panics.iter().take(10).map(|p| format!("\n- {p}")).collect::<String>());
}
