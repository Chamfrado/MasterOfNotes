//! Reference import tests (SPEC-005): JSON valid/invalid, MIDI tracks, tempo map, overlaps.

use scoring_engine::{
    MelodyReference, Note, RefError, import_midi, midi_tracks, parse_reference_json,
    to_reference_json,
};

const SIMPLE: &str = include_str!("../../../tests/fixtures/references/simple.json");
const OVERLAPS: &str = include_str!("../../../tests/fixtures/references/overlaps.json");
const TWO_TRACKS: &[u8] = include_bytes!("../../../tests/fixtures/references/two-tracks.mid");
const TEMPO_CHANGE: &[u8] = include_bytes!("../../../tests/fixtures/references/tempo-change.mid");

fn spans(r: &MelodyReference) -> Vec<(u32, u32, f32)> {
    r.notes
        .iter()
        .map(|n| (n.start_ms, n.dur_ms, n.midi))
        .collect()
}

fn doc(notes: &str) -> String {
    format!(r#"{{"format":"mon-melody","version":1,"notes":[{notes}]}}"#)
}

#[test]
fn parses_simple_json_with_lyrics_and_microtones() {
    let r = parse_reference_json(SIMPLE).unwrap();
    assert_eq!(r.format_version, 1);
    assert_eq!(r.notes.len(), 5);
    assert_eq!(r.notes[0].lyric.as_deref(), Some("Dó"));
    assert_eq!(r.notes[3].midi, 65.5);
    assert_eq!(r.notes[3].lyric, None);
}

#[test]
fn json_round_trips() {
    let r = parse_reference_json(SIMPLE).unwrap();
    assert_eq!(parse_reference_json(&to_reference_json(&r)).unwrap(), r);
}

#[test]
fn overlaps_are_resolved_keeping_the_higher_note() {
    let r = parse_reference_json(OVERLAPS).unwrap();
    assert_eq!(
        spans(&r),
        vec![
            (0, 500, 67.0),    // chord → top note
            (1000, 520, 62.0), // legato: next (lower) note keeps only its tail
            (1520, 280, 60.0),
            (2000, 100, 60.0), // cut by the higher note; remainder dropped
            (2100, 100, 72.0),
        ]
    );
    for w in r.notes.windows(2) {
        assert!(w[0].start_ms + w[0].dur_ms <= w[1].start_ms);
    }
}

#[test]
fn invalid_json_is_rejected_with_a_clear_error() {
    let err = |s: &str| parse_reference_json(s).unwrap_err();
    assert!(matches!(err("{not json"), RefError::Json(_)));
    assert_eq!(
        err(r#"{"format":"x","version":1,"notes":[]}"#),
        RefError::UnknownFormat
    );
    assert_eq!(
        err(r#"{"format":"mon-melody","version":2,"notes":[]}"#),
        RefError::UnsupportedVersion(2)
    );
    assert_eq!(err(&doc("")), RefError::Empty);
    assert_eq!(
        err(&doc(
            r#"{"startMs":0,"durMs":100,"midi":60},{"startMs":200,"durMs":0,"midi":60}"#
        )),
        RefError::ZeroDuration { note: 2 }
    );
    assert_eq!(
        err(&doc(r#"{"startMs":0,"durMs":100,"midi":110}"#)),
        RefError::PitchOutOfRange {
            note: 1,
            midi: 110.0
        }
    );
    assert_eq!(
        err(&doc(r#"{"startMs":0,"durMs":100,"midi":23.9}"#)),
        RefError::PitchOutOfRange {
            note: 1,
            midi: 23.9
        }
    );
    assert!(matches!(
        err(&doc(r#"{"startMs":-5,"durMs":100,"midi":60}"#)),
        RefError::Json(_)
    ));
    // Messages are user-facing pt-BR.
    assert_eq!(
        RefError::ZeroDuration { note: 2 }.to_string(),
        "nota nº 2: a duração deve ser maior que zero"
    );
}

#[test]
fn lists_midi_tracks_with_names_and_note_counts() {
    let tracks = midi_tracks(TWO_TRACKS).unwrap();
    let summary: Vec<_> = tracks
        .iter()
        .map(|t| (t.index, t.name.as_deref(), t.note_count))
        .collect();
    assert_eq!(
        summary,
        vec![
            (0, Some("Tempo"), 0),
            (1, Some("Piano"), 6),
            (2, Some("Vocal"), 4)
        ]
    );
}

#[test]
fn imports_the_chosen_midi_track() {
    let vocal = import_midi(TWO_TRACKS, Some(2)).unwrap();
    assert_eq!(
        spans(&vocal),
        vec![
            (0, 500, 64.0),
            (500, 500, 62.0),
            (1000, 500, 60.0),
            (1500, 500, 62.0)
        ]
    );
    let piano = import_midi(TWO_TRACKS, Some(1)).unwrap();
    assert_eq!(spans(&piano), vec![(0, 1000, 55.0), (1000, 1000, 57.0)]);
    assert_eq!(
        import_midi(TWO_TRACKS, None).unwrap_err(),
        RefError::TrackRequired
    );
    assert_eq!(
        import_midi(TWO_TRACKS, Some(9)).unwrap_err(),
        RefError::NoSuchTrack(9)
    );
    assert_eq!(
        import_midi(TWO_TRACKS, Some(0)).unwrap_err(),
        RefError::Empty
    );
}

#[test]
fn midi_tempo_change_is_honoured_to_the_millisecond() {
    // 120 BPM (500 ms/beat) until tick 1920 = 2000 ms, then 60 BPM (1000 ms/beat).
    let r = import_midi(TEMPO_CHANGE, None).unwrap();
    assert_eq!(
        spans(&r),
        vec![
            (0, 250, 60.0),
            (1000, 250, 62.0),
            (1750, 750, 64.0), // crosses the change: 250 ms @120 + 500 ms @60
            (3000, 500, 65.0),
            (4000, 500, 67.0),
        ]
    );
}

#[test]
fn smpte_and_garbage_midi_are_rejected() {
    // Header with SMPTE division (−25 fps, 40 ticks/frame) and one empty track.
    let mut smpte = b"MThd\0\0\0\x06\0\0\0\x01\xE7\x28".to_vec();
    smpte.extend_from_slice(b"MTrk\0\0\0\x04\0\xFF\x2F\0");
    assert_eq!(midi_tracks(&smpte).unwrap_err(), RefError::SmpteUnsupported);
    assert_eq!(
        import_midi(&smpte, None).unwrap_err(),
        RefError::SmpteUnsupported
    );
    assert!(matches!(
        import_midi(b"not a midi file", None),
        Err(RefError::Midi(_))
    ));
}

#[test]
fn notes_are_sorted_on_import() {
    let r = parse_reference_json(&doc(
        r#"{"startMs":900,"durMs":100,"midi":60},{"startMs":100,"durMs":100,"midi":62}"#,
    ))
    .unwrap();
    assert_eq!(
        r.notes,
        vec![
            Note {
                start_ms: 100,
                dur_ms: 100,
                midi: 62.0,
                lyric: None
            },
            Note {
                start_ms: 900,
                dur_ms: 100,
                midi: 60.0,
                lyric: None
            },
        ]
    );
}
