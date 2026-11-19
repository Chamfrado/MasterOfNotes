//! Melody references and deterministic vocal scoring. Owner: Agent 2. Specs: SPEC-005, SPEC-006.
//! Pure: no I/O, no randomness, no threads.

mod reference;

pub use reference::{
    MelodyReference, MidiTrackInfo, Note, REFERENCE_FORMAT_VERSION, RefError, import_midi,
    midi_tracks, parse_reference_json, to_reference_json,
};
