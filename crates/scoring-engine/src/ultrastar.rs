//! UltraStar `.txt` import (SPEC-013, ADR-0002).
//!
//! - Header `#KEY:VALUE`: `#BPM` (required; decimal comma allowed), `#GAP` (ms,
//!   default 0), `#TITLE`, `#ARTIST`, `#RELATIVE:yes`, `#ENCODING`.
//! - Notes `T start length pitch syllable`: `:` normal and `*` golden become
//!   notes; `F` (freestyle), `R`/`G` (rap) have no pitch to score and are left
//!   out; `-` line breaks and `E` (end) are structure; `P1`/`P2`/`P3` mark duet
//!   singers (only `P1`, and `P3` = both, are kept).
//! - Time: `ms = GAP + beat × 60000 / (BPM × 4)`; pitch 0 = C4 = MIDI 60.
//! - Text: UTF-8 (with or without BOM); invalid UTF-8 or `#ENCODING:CP1252`
//!   is read as Windows-1252 (older files).
//!
//! The result goes through the same validation/skyline as the other sources.

use serde::Serialize;

use crate::reference::{MelodyReference, Note, RefError, validate};

/// What the file says about itself (informative; shown by the UI).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UltraStarInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    /// The file has a second singer (`P2`); only `P1` was imported.
    pub duet: bool,
}

fn line_err(line: usize, reason: impl Into<String>) -> RefError {
    RefError::UltraStarLine {
        line,
        reason: reason.into(),
    }
}

/// Imports the melody of an UltraStar `.txt` (only the first singer of a duet).
pub fn import_ultrastar(bytes: &[u8]) -> Result<(MelodyReference, UltraStarInfo), RefError> {
    let text = decode(bytes);
    let mut info = UltraStarInfo::default();
    let (mut bpm, mut gap_ms, mut relative) = (None::<f64>, 0.0f64, false);
    // Beats added to note positions (RELATIVE: the sum of the line breaks).
    let mut base: i64 = 0;
    // `None` = no player marker yet (not a duet so far).
    let mut player: Option<u8> = None;
    let mut notes: Vec<Note> = Vec::new();

    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            continue;
        }
        if let Some(h) = line.strip_prefix('#') {
            let (key, value) = h.split_once(':').unwrap_or((h, ""));
            let value = value.trim();
            match key.trim().to_ascii_uppercase().as_str() {
                "BPM" => {
                    let v = number(value).filter(|v| *v > 0.0).ok_or_else(|| {
                        line_err(n, format!("#BPM \"{value}\" não é um número positivo"))
                    })?;
                    bpm = Some(v);
                }
                "GAP" => {
                    gap_ms = number(value)
                        .ok_or_else(|| line_err(n, format!("#GAP \"{value}\" não é um número")))?;
                }
                "RELATIVE" => relative = value.eq_ignore_ascii_case("yes"),
                "TITLE" if !value.is_empty() => info.title = Some(value.to_string()),
                "ARTIST" if !value.is_empty() => info.artist = Some(value.to_string()),
                _ => {}
            }
            continue;
        }
        let trimmed = line.trim_start();
        let kind = trimmed.chars().next().unwrap_or(' ');
        let rest = &trimmed[kind.len_utf8()..];
        match kind {
            'E' => break,
            'P' => {
                let p = rest.trim();
                player = match p {
                    "1" => Some(1),
                    "2" => Some(2),
                    "3" => Some(3),
                    _ => return Err(line_err(n, format!("cantor \"P{p}\" desconhecido"))),
                };
                info.duet |= player == Some(2);
            }
            '-' => {
                if relative {
                    let mut it = rest.split_whitespace();
                    let first = it.next().map(|t| integer(t, n)).transpose()?;
                    let second = it.next().map(|t| integer(t, n)).transpose()?;
                    base += second.or(first).ok_or_else(|| {
                        line_err(n, "quebra de linha sem a batida (modo RELATIVE)")
                    })?;
                }
            }
            ':' | '*' | 'F' | 'R' | 'G' => {
                let (start, rest) = field(rest, n, "início")?;
                let (length, rest) = field(rest, n, "duração")?;
                let (pitch, rest) = field(rest, n, "altura")?;
                let keep = matches!(kind, ':' | '*') && player.is_none_or(|p| p != 2);
                if !keep {
                    continue;
                }
                let Some(bpm) = bpm else {
                    return Err(line_err(n, "nota antes do #BPM"));
                };
                if length <= 0 {
                    return Err(line_err(
                        n,
                        format!("duração {length} (deve ser maior que zero)"),
                    ));
                }
                let midi = 60 + pitch;
                if !(24..=108).contains(&midi) {
                    return Err(line_err(
                        n,
                        format!("altura {pitch} (MIDI {midi}) fora da faixa 24–108"),
                    ));
                }
                let ms = |beat: i64| gap_ms + (base + beat) as f64 * 60_000.0 / (bpm * 4.0);
                let (a, b) = (ms(start).round(), ms(start + length).round());
                if a < 0.0 || b > u32::MAX as f64 {
                    return Err(line_err(n, "a nota cai fora da música (antes de 0 ms)"));
                }
                let syllable = rest.trim().trim_start_matches('~').trim();
                notes.push(Note {
                    start_ms: a as u32,
                    dur_ms: (b - a).max(1.0) as u32,
                    midi: midi as f32,
                    lyric: (!syllable.is_empty()).then(|| syllable.to_string()),
                });
            }
            _ => {
                return Err(line_err(
                    n,
                    format!(
                        "linha não reconhecida: \"{}\"",
                        line.chars().take(40).collect::<String>()
                    ),
                ));
            }
        }
    }
    if bpm.is_none() {
        return Err(RefError::UltraStarHeader("falta o #BPM".into()));
    }
    Ok((validate(notes)?, info))
}

/// Number with a decimal point or comma (`300`, `271,5`).
fn number(s: &str) -> Option<f64> {
    s.trim()
        .replace(',', ".")
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
}

fn integer(t: &str, line: usize) -> Result<i64, RefError> {
    t.parse()
        .map_err(|_| line_err(line, format!("\"{t}\" não é um número inteiro")))
}

/// Next whitespace-separated integer of a note line, and the text after it.
fn field<'a>(s: &'a str, line: usize, what: &str) -> Result<(i64, &'a str), RefError> {
    let s = s.trim_start();
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    if end == 0 {
        return Err(line_err(line, format!("falta a {what} da nota")));
    }
    let v = s[..end].parse().map_err(|_| {
        line_err(
            line,
            format!("{what} \"{}\" não é um número inteiro", &s[..end]),
        )
    })?;
    Ok((v, &s[end..]))
}

/// UTF-8 (BOM stripped) unless the bytes are not UTF-8 or the header says
/// CP1252/Latin-1; then Windows-1252.
fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let declared_1252 = declared_encoding(bytes).is_some_and(|e| {
        let e = e.to_ascii_uppercase().replace(['-', '_', ' '], "");
        matches!(e.as_str(), "CP1252" | "WINDOWS1252" | "LATIN1" | "ISO88591")
    });
    match std::str::from_utf8(bytes) {
        Ok(s) if !declared_1252 => s.to_string(),
        _ => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

/// Value of an `#ENCODING:` header line (ASCII), if any.
fn declared_encoding(bytes: &[u8]) -> Option<String> {
    bytes.split(|&b| b == b'\n').find_map(|l| {
        let l = String::from_utf8_lossy(l);
        let l = l.trim();
        l.get(..10)
            .filter(|k| k.eq_ignore_ascii_case("#ENCODING:"))
            .map(|_| l[10..].trim().to_string())
    })
}

/// Windows-1252 byte → char (WHATWG mapping; undefined bytes map to C1 controls).
fn cp1252(b: u8) -> char {
    const HIGH: [u16; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013,
        0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    match b {
        0x80..=0x9F => char::from_u32(HIGH[(b - 0x80) as usize] as u32).unwrap_or('\u{FFFD}'),
        _ => b as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp1252_maps_the_special_range_and_latin1() {
        assert_eq!(cp1252(0x80), '€');
        assert_eq!(cp1252(0x93), '\u{201C}');
        assert_eq!(cp1252(0xE1), 'á');
        assert_eq!(cp1252(b'A'), 'A');
    }

    #[test]
    fn decode_prefers_utf8_unless_declared_or_invalid() {
        assert_eq!(decode("\u{FEFF}Lá".as_bytes()), "Lá");
        assert_eq!(decode(b"L\xE1"), "Lá");
        assert_eq!(
            decode(b"#ENCODING:CP1252\nL\xC3\xA1"),
            "#ENCODING:CP1252\nLÃ¡"
        );
    }
}
