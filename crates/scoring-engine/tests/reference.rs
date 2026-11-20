//! Reference import tests (SPEC-005, SPEC-013): JSON valid/invalid, MIDI tracks, tempo map,
//! overlaps, UltraStar .txt.

use scoring_engine::{
    MelodyReference, Note, RefError, UltraStarInfo, import_midi, import_ultrastar, midi_tracks,
    parse_reference_json, to_reference_json,
};

const SIMPLE: &str = include_str!("../../../tests/fixtures/references/simple.json");
const OVERLAPS: &str = include_str!("../../../tests/fixtures/references/overlaps.json");
const TWO_TRACKS: &[u8] = include_bytes!("../../../tests/fixtures/references/two-tracks.mid");
const TEMPO_CHANGE: &[u8] = include_bytes!("../../../tests/fixtures/references/tempo-change.mid");
const US_SIMPLE: &[u8] = include_bytes!("../../../tests/fixtures/references/ultrastar-simple.txt");
const US_CP1252: &[u8] =
    include_bytes!("../../../tests/fixtures/references/ultrastar-cp1252-relative.txt");
const US_DUET: &[u8] = include_bytes!("../../../tests/fixtures/references/ultrastar-duet.txt");

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

// ---------- UltraStar .txt (SPEC-013, T-013-01) ----------

fn lyrics(r: &MelodyReference) -> Vec<Option<&str>> {
    r.notes.iter().map(|n| n.lyric.as_deref()).collect()
}

#[test]
fn ultrastar_spec_example_bpm_gap_and_pitch() {
    // SPEC-013: #BPM:300, #GAP:1000, ": 0 4 0 Lá" → 1000 ms, 200 ms, MIDI 60, "Lá".
    let (r, _) = import_ultrastar("#BPM:300\n#GAP:1000\n: 0 4 0 Lá\nE\n".as_bytes()).unwrap();
    assert_eq!(spans(&r), vec![(1000, 200, 60.0)]);
    assert_eq!(lyrics(&r), vec![Some("Lá")]);
}

#[test]
fn ultrastar_simple_file_golden_freestyle_rap_and_end() {
    let (r, info) = import_ultrastar(US_SIMPLE).unwrap();
    assert_eq!(
        info,
        UltraStarInfo {
            title: Some("Escala do Teste".into()),
            artist: Some("Master Of Notes".into()),
            duet: false
        }
    );
    // F, R and G lines are left out; the note after E is ignored; "*" is a note.
    assert_eq!(
        spans(&r),
        vec![
            (1000, 200, 60.0),
            (1200, 200, 62.0),
            (1400, 400, 64.0),
            (1800, 100, 65.0),
            (1900, 100, 65.0),
            (2600, 400, 67.0),
            (3200, 600, 48.0),
        ]
    );
    // Syllables are trimmed; "~" (continuation) has no text of its own.
    assert_eq!(
        lyrics(&r),
        vec![
            Some("Lá"),
            Some("vai"),
            Some("a"),
            Some("es"),
            None,
            Some("ca"),
            Some("fim")
        ]
    );
}

#[test]
fn ultrastar_cp1252_relative_with_decimal_comma_bpm() {
    // BPM 250,0 → 60 ms per beat; GAP 500; RELATIVE line breaks add 12 then 20 beats.
    let (r, info) = import_ultrastar(US_CP1252).unwrap();
    assert_eq!(info.title.as_deref(), Some("Canção do Teste"));
    assert_eq!(info.artist.as_deref(), Some("Autoria própria"));
    assert_eq!(
        spans(&r),
        vec![
            (500, 240, 60.0),
            (740, 240, 62.0),
            (1220, 240, 64.0),
            (1460, 360, 65.0),
            (1820, 240, 67.0),
            (2420, 480, 60.0),
        ]
    );
    assert_eq!(
        lyrics(&r),
        vec![
            Some("Can"),
            Some("ção"),
            Some("co"),
            Some("ra"),
            Some("ção"),
            Some("é")
        ]
    );
}

#[test]
fn ultrastar_duet_keeps_only_the_first_singer() {
    let (r, info) = import_ultrastar(US_DUET).unwrap();
    assert!(info.duet);
    // P1 notes and the P3 (both singers) note; P2 is left out.
    assert_eq!(
        spans(&r),
        vec![(0, 200, 60.0), (200, 200, 62.0), (600, 400, 64.0)]
    );
    assert_eq!(lyrics(&r), vec![Some("eu"), Some("can"), Some("nós")]);
}

#[test]
fn ultrastar_crlf_and_utf8_bom_are_accepted() {
    // CRLF is tested inline: the repo normalises fixture line endings to LF.
    let (r, _) =
        import_ultrastar(b"\xEF\xBB\xBF#BPM:300\r\n#GAP:0\r\n: 0 4 0 a\r\n: 4 4 2 b\r\nE\r\n")
            .unwrap();
    assert_eq!(spans(&r), vec![(0, 200, 60.0), (200, 200, 62.0)]);
    assert_eq!(lyrics(&r), vec![Some("a"), Some("b")]);
}

#[test]
fn ultrastar_errors_name_the_line_in_portuguese() {
    let err = |s: &str| import_ultrastar(s.as_bytes()).unwrap_err();
    assert_eq!(
        err("#BPM:300\n: 0 4 x la\n"),
        RefError::UltraStarLine {
            line: 2,
            reason: "altura \"x\" não é um número inteiro".into()
        }
    );
    assert_eq!(
        err("#BPM:300\n: 0 4 x la\n").to_string(),
        "arquivo UltraStar, linha 2: altura \"x\" não é um número inteiro"
    );
    assert_eq!(
        err("#GAP:0\nE\n"),
        RefError::UltraStarHeader("falta o #BPM".into())
    );
    for (text, line) in [
        ("#BPM:abc\n", 1),                         // BPM not a number
        ("#BPM:300\n: 0 0 0 la\n", 2),             // zero length
        ("#BPM:300\n\n: 0 4 60 la\n", 3),          // MIDI 120, out of range
        ("#BPM:300\n: 0 4\n", 2),                  // missing pitch
        ("#BPM:300\nxyz\n", 2),                    // unknown line
        ("#BPM:300\nP9\n", 2),                     // unknown singer
        ("#BPM:300\n#GAP:-5000\n: 0 4 0 la\n", 3), // before 0 ms
        (": 0 4 0 la\n#BPM:300\n", 1),             // note before #BPM
    ] {
        match err(text) {
            RefError::UltraStarLine { line: l, .. } => assert_eq!(l, line, "{text:?}"),
            e => panic!("{text:?}: {e:?}"),
        }
    }
    // Only freestyle/rap: nothing to score.
    assert_eq!(
        err("#BPM:300\nF 0 4 0 la\nR 4 4 0 la\nE\n"),
        RefError::Empty
    );
}

#[test]
fn ultrastar_goes_through_the_same_validation_and_skyline() {
    // Overlapping notes (a lower one under a higher one) are resolved like MIDI/JSON.
    let (r, _) = import_ultrastar(b"#BPM:300\n: 0 8 0 a\n: 4 8 5 b\nE\n").unwrap();
    assert_eq!(spans(&r), vec![(0, 200, 60.0), (200, 400, 65.0)]);
    // The result round-trips through the stored format.
    let back = parse_reference_json(&to_reference_json(&r)).unwrap();
    assert_eq!(back, r);
}

#[test]
fn ultrastar_extreme_numbers_never_panic() {
    // Largest values that parse: sums are done in i64, results are errors or notes.
    for text in [
        "#BPM:300\n: 2147483647 2147483647 0 la\n",
        "#BPM:300\n: -2147483648 2147483647 0 la\n",
        "#BPM:300\n#RELATIVE:yes\n- 2147483647 2147483647\n- 2147483647 2147483647\n: 2147483647 1 0 la\n",
        "#BPM:0,0001\n: 0 2147483647 0 la\n",
        "#BPM:1e300\n#GAP:1e300\n: 0 1 0 la\n",
    ] {
        let _ = import_ultrastar(text.as_bytes());
    }
}
