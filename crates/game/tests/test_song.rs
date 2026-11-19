//! T-003-05: the test-song reference (apps/desktop/e2e/tools) imports with the real parser and has the
//! features the owner's manual check relies on. Audio/reference alignment is measured in e2e/test-song.e2e.ts.

use scoring_engine::parse_reference_json;

const JSON: &str = include_str!("../../../apps/desktop/e2e/tools/test-song.mon-melody.json");

#[test]
fn test_song_reference_parses_and_has_the_planned_features() {
    let r = parse_reference_json(JSON).expect("parse_reference_json");
    let n = &r.notes;
    assert_eq!(n.len(), 25, "the validator must not drop or merge notes");
    let end = |i: usize| n[i].start_ms + n[i].dur_ms;
    assert!(n[0].start_ms >= 2000, "2 s instrumental intro");
    assert!(
        (1..n.len()).any(|i| n[i].midi == n[i - 1].midi && n[i].start_ms == end(i - 1)),
        "two repeated legato notes"
    );
    assert!(
        n.iter().any(|x| (120..=180).contains(&x.dur_ms)),
        "a ~150 ms note"
    );
    assert!(
        (1..n.len()).any(|i| n[i].start_ms - end(i - 1) >= 3000),
        "an instrumental pause"
    );
    assert!(
        end(n.len() - 1) <= 31_000 - 2000,
        "ends ≥ 2 s before SONG_MS (31 s, test-song.ts)"
    );
}
