//! Melody references and deterministic vocal scoring. Owner: Agent 2. Specs: SPEC-005, SPEC-006, SPEC-013.
//! Pure: no I/O, no randomness, no threads.

mod exploratory;
mod reference;
mod scorer;
mod segment;
mod ultrastar;

pub use exploratory::{ExploratoryMetrics, exploratory_metrics};
pub use reference::{
    MelodyReference, MidiTrackInfo, Note, REFERENCE_FORMAT_VERSION, RefError, import_midi,
    midi_tracks, parse_reference_json, to_reference_json,
};
pub use scorer::{
    ALGORITHM_VERSION, Difficulty, FeedbackLabel, LiveFeedback, ScoreResult, Scorer, VoiceFrame,
};
pub use segment::segment_notes;
pub use ultrastar::{UltraStarInfo, import_ultrastar};
