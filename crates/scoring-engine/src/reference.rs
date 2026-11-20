//! Melody references: `mon-melody` v1 JSON and Standard MIDI File import (SPEC-005, ADR-0002).
//! UltraStar `.txt` import lives in `ultrastar.rs` (SPEC-013).

use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const REFERENCE_FORMAT_VERSION: u32 = 1;
const FORMAT_NAME: &str = "mon-melody";
const MIDI_RANGE: std::ops::RangeInclusive<f32> = 24.0..=108.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub start_ms: u32,
    pub dur_ms: u32,
    /// May be fractional (microtonal).
    pub midi: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyric: Option<String>,
}

impl Note {
    fn end_ms(&self) -> u64 {
        self.start_ms as u64 + self.dur_ms as u64
    }
}

/// Validated reference: ≥ 1 note, sorted by start, no overlaps, MIDI 24–108.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MelodyReference {
    pub format_version: u32,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiTrackInfo {
    pub index: usize,
    pub name: Option<String>,
    pub note_count: u32,
}

/// Messages are user-facing (pt-BR) so the IPC layer can pass them through.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RefError {
    #[error("JSON inválido: {0}")]
    Json(String),
    #[error("formato não reconhecido (esperado \"mon-melody\")")]
    UnknownFormat,
    #[error("versão {0} do formato mon-melody não suportada (suportada: 1)")]
    UnsupportedVersion(u32),
    #[error("a referência não tem notas")]
    Empty,
    #[error("nota nº {note}: a duração deve ser maior que zero")]
    ZeroDuration { note: usize },
    #[error("nota nº {note}: altura MIDI {midi} fora da faixa 24–108")]
    PitchOutOfRange { note: usize, midi: f32 },
    #[error("arquivo MIDI inválido: {0}")]
    Midi(String),
    #[error("formato não suportado: MIDI com timecode SMPTE")]
    SmpteUnsupported,
    #[error("formato não suportado: MIDI tipo 2 (sequências independentes)")]
    SequentialUnsupported,
    #[error("a trilha {0} não existe no arquivo MIDI")]
    NoSuchTrack(usize),
    #[error("o arquivo MIDI tem várias trilhas com notas; escolha a trilha vocal")]
    TrackRequired,
    #[error("arquivo UltraStar inválido: {0}")]
    UltraStarHeader(String),
    #[error("arquivo UltraStar, linha {line}: {reason}")]
    UltraStarLine { line: usize, reason: String },
}

#[derive(Serialize, Deserialize)]
struct MonMelodyFile {
    format: String,
    version: u32,
    notes: Vec<Note>,
}

/// Parses and validates a `mon-melody` v1 document.
pub fn parse_reference_json(json: &str) -> Result<MelodyReference, RefError> {
    let file: MonMelodyFile =
        serde_json::from_str(json).map_err(|e| RefError::Json(e.to_string()))?;
    if file.format != FORMAT_NAME {
        return Err(RefError::UnknownFormat);
    }
    if file.version != REFERENCE_FORMAT_VERSION {
        return Err(RefError::UnsupportedVersion(file.version));
    }
    validate(file.notes)
}

/// Serializes a reference as a `mon-melody` v1 document (inverse of [`parse_reference_json`]).
pub fn to_reference_json(reference: &MelodyReference) -> String {
    let file = MonMelodyFile {
        format: FORMAT_NAME.into(),
        version: reference.format_version,
        notes: reference.notes.clone(),
    };
    // Serializing plain structs of numbers/strings cannot fail.
    serde_json::to_string(&file).unwrap_or_default()
}

/// Validates (SPEC-005 R2) and normalises notes into a monophonic, sorted line.
///
/// Overlaps keep the higher note ("skyline"): a higher note cuts the one under
/// it; a lower note that starts under a higher one keeps only the part after the
/// higher note ends (legato tails shift by a few ms) or is dropped if fully
/// covered. Same-start chords keep the top note.
pub(crate) fn validate(notes: Vec<Note>) -> Result<MelodyReference, RefError> {
    if notes.is_empty() {
        return Err(RefError::Empty);
    }
    for (i, n) in notes.iter().enumerate() {
        if n.dur_ms == 0 {
            return Err(RefError::ZeroDuration { note: i + 1 });
        }
        if !MIDI_RANGE.contains(&n.midi) {
            return Err(RefError::PitchOutOfRange {
                note: i + 1,
                midi: n.midi,
            });
        }
    }
    let mut sorted = notes;
    sorted.sort_by(|a, b| a.start_ms.cmp(&b.start_ms).then(b.midi.total_cmp(&a.midi)));

    let mut out: Vec<Note> = Vec::with_capacity(sorted.len());
    'notes: for mut n in sorted {
        while let Some(k) = out.last_mut() {
            let k_end = k.end_ms();
            if n.start_ms as u64 >= k_end {
                break;
            }
            if n.midi > k.midi {
                if n.start_ms > k.start_ms {
                    k.dur_ms = n.start_ms - k.start_ms;
                    break;
                }
                out.pop(); // fully replaced by the higher note
            } else {
                if n.end_ms() <= k_end {
                    continue 'notes; // lower note fully covered: drop
                }
                n.dur_ms = (n.end_ms() - k_end) as u32;
                n.start_ms = u32::try_from(k_end).unwrap_or(u32::MAX);
                break;
            }
        }
        out.push(n);
    }
    Ok(MelodyReference {
        format_version: REFERENCE_FORMAT_VERSION,
        notes: out,
    })
}

/// Lists the tracks of a MIDI file so the user can pick the vocal one.
pub fn midi_tracks(bytes: &[u8]) -> Result<Vec<MidiTrackInfo>, RefError> {
    let (smf, map) = parse_smf(bytes)?;
    Ok(smf
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| MidiTrackInfo {
            index,
            name: track.iter().find_map(|e| match e.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(n)) => {
                    Some(String::from_utf8_lossy(n).trim().to_string())
                }
                _ => None,
            }),
            note_count: track_notes(track, &map).len() as u32,
        })
        .collect())
}

/// Imports one track as a reference. `track = None` picks the only track with
/// notes and fails with [`RefError::TrackRequired`] if there are several.
pub fn import_midi(bytes: &[u8], track: Option<usize>) -> Result<MelodyReference, RefError> {
    let (smf, map) = parse_smf(bytes)?;
    let notes = match track {
        Some(i) => track_notes(smf.tracks.get(i).ok_or(RefError::NoSuchTrack(i))?, &map),
        None => {
            let mut with_notes = smf
                .tracks
                .iter()
                .map(|t| track_notes(t, &map))
                .filter(|n| !n.is_empty());
            let first = with_notes.next().ok_or(RefError::Empty)?;
            if with_notes.next().is_some() {
                return Err(RefError::TrackRequired);
            }
            first
        }
    };
    validate(notes)
}

fn parse_smf(bytes: &[u8]) -> Result<(Smf<'_>, TempoMap), RefError> {
    let smf = Smf::parse(bytes).map_err(|e| RefError::Midi(e.to_string()))?;
    let Timing::Metrical(tpb) = smf.header.timing else {
        return Err(RefError::SmpteUnsupported);
    };
    if smf.header.format == Format::Sequential {
        return Err(RefError::SequentialUnsupported);
    }
    let mut changes = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0u64;
        for e in track {
            tick += e.delta.as_int() as u64;
            if let TrackEventKind::Meta(MetaMessage::Tempo(t)) = e.kind {
                changes.push((tick, t.as_int()));
            }
        }
    }
    let map = TempoMap::new(changes, tpb.as_int().max(1));
    Ok((smf, map))
}

/// Tick → microseconds, honouring every tempo change (default 120 BPM).
struct TempoMap {
    /// (tick, µs at tick, µs per tick from here on)
    segments: Vec<(u64, f64, f64)>,
}

impl TempoMap {
    fn new(mut changes: Vec<(u64, u32)>, tpb: u16) -> Self {
        changes.sort_by_key(|c| c.0); // stable: later events at the same tick win
        let rate = |us_per_beat: u32| us_per_beat as f64 / tpb as f64;
        let mut segments = vec![(0u64, 0.0f64, rate(500_000))];
        for (tick, tempo) in changes {
            let &(t0, us0, r0) = segments.last().unwrap_or(&(0, 0.0, 0.0));
            if tick == t0 {
                segments.pop();
                segments.push((t0, us0, rate(tempo)));
            } else {
                segments.push((tick, us0 + (tick - t0) as f64 * r0, rate(tempo)));
            }
        }
        Self { segments }
    }

    fn ms(&self, tick: u64) -> u32 {
        let i = self
            .segments
            .partition_point(|s| s.0 <= tick)
            .saturating_sub(1);
        let (t0, us0, r) = self.segments[i];
        ((us0 + (tick - t0) as f64 * r) / 1000.0).round() as u32
    }
}

/// Note-on/note-off pairing per (channel, key), FIFO; velocity-0 note-on = off.
/// Notes still open at the end of the track end at its last event. Notes that
/// round to 0 ms are dropped (MIDI artefacts, not singable).
fn track_notes(track: &[midly::TrackEvent<'_>], map: &TempoMap) -> Vec<Note> {
    let mut open: HashMap<(u8, u8), Vec<u64>> = HashMap::new();
    let mut spans = Vec::new();
    let mut tick = 0u64;
    for e in track {
        tick += e.delta.as_int() as u64;
        let TrackEventKind::Midi { channel, message } = e.kind else {
            continue;
        };
        match message {
            MidiMessage::NoteOn { key, vel } if vel > 0 => {
                open.entry((channel.as_int(), key.as_int()))
                    .or_default()
                    .push(tick);
            }
            MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                if let Some(starts) = open.get_mut(&(channel.as_int(), key.as_int()))
                    && !starts.is_empty()
                {
                    spans.push((starts.remove(0), tick, key.as_int()));
                }
            }
            _ => {}
        }
    }
    for ((_, key), starts) in open {
        spans.extend(starts.into_iter().map(|s| (s, tick, key)));
    }
    spans.sort_unstable();
    spans
        .into_iter()
        .filter_map(|(s, e, key)| {
            let (start_ms, end_ms) = (map.ms(s), map.ms(e));
            (end_ms > start_ms).then(|| Note {
                start_ms,
                dur_ms: end_ms - start_ms,
                midi: key as f32,
                lyric: None,
            })
        })
        .collect()
}
