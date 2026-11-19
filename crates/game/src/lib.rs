//! Game rules: sessions, rounds, standings, leaderboard compatibility. Owner: Agent 4. Specs: SPEC-007, SPEC-008.
//!
//! Pure functions, no I/O. Persistence and name lookup live in `src-tauri`.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

/// Max rows returned by [`leaderboard`] (SPEC-007 R4).
pub const LEADERBOARD_SIZE: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameMode {
    Solo,
    Party,
    Duel,
    Challenge,
}

/// Scores are only comparable when every field matches (SPEC-007 R3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatKey {
    pub reference_id: i64,
    pub reference_revision: i64,
    pub algorithm_version: u32,
    pub difficulty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceRecord {
    /// `None` = anonymous (Solo without a name).
    pub participant_id: Option<i64>,
    pub song_id: String,
    pub score: f32,
    pub compat: CompatKey,
    /// Unix epoch ms; older wins ties in [`leaderboard`].
    pub created_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Standing {
    pub participant_id: i64,
    pub total: f32,
    pub average: f32,
    pub rounds: u32,
    /// `rounds >= min_rounds`. Ineligible participants rank below every eligible one.
    pub eligible: bool,
    /// 1-based, sequential over the returned list.
    pub rank: u32,
}

/// Party standings (SPEC-008): eligible first, then average desc, total desc, id asc
/// (ids are assigned in registration order). Anonymous records are ignored.
pub fn party_standings(records: &[PerformanceRecord], min_rounds: u32) -> Vec<Standing> {
    let mut standings: Vec<Standing> = Vec::new();
    for r in records {
        let Some(id) = r.participant_id else { continue };
        match standings.iter_mut().find(|s| s.participant_id == id) {
            Some(s) => {
                s.total += r.score;
                s.rounds += 1;
            }
            None => standings.push(Standing {
                participant_id: id,
                total: r.score,
                average: 0.0,
                rounds: 1,
                eligible: false,
                rank: 0,
            }),
        }
    }
    for s in &mut standings {
        s.average = s.total / s.rounds as f32;
        s.eligible = s.rounds >= min_rounds;
    }
    standings.sort_by(|a, b| {
        b.eligible
            .cmp(&a.eligible)
            .then(b.average.total_cmp(&a.average))
            .then(b.total.total_cmp(&a.total))
            .then(a.participant_id.cmp(&b.participant_id))
    });
    for (i, s) in standings.iter_mut().enumerate() {
        s.rank = i as u32 + 1;
    }
    standings
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoundOrder {
    /// Keep the given order of participants and songs.
    Manual,
    /// Shuffle participants and songs deterministically from `seed`.
    Shuffled,
}

/// Plans `max(participants, songs)` rounds; round `i` = `(participants[i % p], songs[i % s])`,
/// so everyone sings at least once and every song is used at least once.
/// Same inputs + seed => same plan. Empty input => no rounds.
pub fn plan_rounds(
    participants: &[i64],
    songs: &[String],
    order: RoundOrder,
    seed: u64,
) -> Vec<(i64, String)> {
    if participants.is_empty() || songs.is_empty() {
        return Vec::new();
    }
    let mut people = participants.to_vec();
    let mut tracks = songs.to_vec();
    if order == RoundOrder::Shuffled {
        let mut state = seed;
        shuffle(&mut people, &mut state);
        shuffle(&mut tracks, &mut state);
    }
    (0..people.len().max(tracks.len()))
        .map(|i| (people[i % people.len()], tracks[i % tracks.len()].clone()))
        .collect()
}

/// splitmix64: tiny, well-distributed, stable across platforms and releases.
fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Fisher-Yates. ponytail: modulo bias is ~n/2^64, irrelevant for party-sized lists.
fn shuffle<T>(items: &mut [T], state: &mut u64) {
    for i in (1..items.len()).rev() {
        let j = (next_u64(state) % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GameError {
    #[error(
        "as performances não são comparáveis (referência, revisão, algoritmo ou dificuldade diferentes)"
    )]
    IncompatibleRecords,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuelOutcome {
    /// `None` = draw (equal after rounding to 0.1) or the winner sang anonymously.
    pub winner: Option<i64>,
    /// `|round(a) - round(b)|`, scores rounded to 0.1. Zero iff draw.
    pub margin: f32,
}

/// Duel result. Errors when the two records have different [`CompatKey`]s (SPEC-008).
pub fn duel_outcome(
    a: &PerformanceRecord,
    b: &PerformanceRecord,
) -> Result<DuelOutcome, GameError> {
    if a.compat != b.compat {
        return Err(GameError::IncompatibleRecords);
    }
    // Compare in integer tenths (SPEC-006 display precision) so draw <=> margin == 0.
    let tenths = |s: f32| (s * 10.0).round() as i64;
    let (ta, tb) = (tenths(a.score), tenths(b.score));
    let winner = match ta.cmp(&tb) {
        Ordering::Greater => a.participant_id,
        Ordering::Less => b.participant_id,
        Ordering::Equal => None,
    };
    Ok(DuelOutcome {
        winner,
        margin: (ta - tb).abs() as f32 / 10.0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChallengeStatus {
    InProgress,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeProgress {
    pub completed: u32,
    pub remaining: u32,
    /// Average of the completed rounds (0 when none).
    pub average: f32,
    pub status: ChallengeStatus,
}

/// Challenge progress: decided only after `rounds` scores; success = average >= target.
/// Scores beyond `rounds` are ignored.
pub fn challenge_progress(scores: &[f32], target: f32, rounds: u32) -> ChallengeProgress {
    let done = &scores[..scores.len().min(rounds as usize)];
    let completed = done.len() as u32;
    let average = if done.is_empty() {
        0.0
    } else {
        done.iter().sum::<f32>() / completed as f32
    };
    let status = if completed < rounds {
        ChallengeStatus::InProgress
    } else if average >= target {
        ChallengeStatus::Succeeded
    } else {
        ChallengeStatus::Failed
    };
    ChallengeProgress {
        completed,
        remaining: rounds - completed,
        average,
        status,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardRow {
    /// 1-based.
    pub position: u32,
    /// `None` = "Anônimo".
    pub participant_id: Option<i64>,
    pub score: f32,
    pub created_ms: i64,
}

/// Song leaderboard (SPEC-007 R3/R4): only records with `compat`, score desc, ties older first,
/// top [`LEADERBOARD_SIZE`]. Caller passes competitive performances only.
pub fn leaderboard(records: &[PerformanceRecord], compat: &CompatKey) -> Vec<LeaderboardRow> {
    let mut rows: Vec<&PerformanceRecord> =
        records.iter().filter(|r| &r.compat == compat).collect();
    rows.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.created_ms.cmp(&b.created_ms))
    });
    rows.iter()
        .take(LEADERBOARD_SIZE)
        .enumerate()
        .map(|(i, r)| LeaderboardRow {
            position: i as u32 + 1,
            participant_id: r.participant_id,
            score: r.score,
            created_ms: r.created_ms,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(difficulty: &str) -> CompatKey {
        CompatKey {
            reference_id: 1,
            reference_revision: 1,
            algorithm_version: 1,
            difficulty: difficulty.into(),
        }
    }

    fn rec(participant: i64, score: f32) -> PerformanceRecord {
        rec_at(Some(participant), score, 0)
    }

    fn rec_at(participant: Option<i64>, score: f32, created_ms: i64) -> PerformanceRecord {
        PerformanceRecord {
            participant_id: participant,
            song_id: "song".into(),
            score,
            compat: key("normal"),
            created_ms,
        }
    }

    fn songs(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("s{i}")).collect()
    }

    // SPEC-008: A 3 rounds (avg 80, total 240), B 2 rounds (avg 90, total 180), minRounds 2 → B first.
    #[test]
    fn average_beats_total() {
        let r = [
            rec(1, 80.0),
            rec(1, 80.0),
            rec(1, 80.0),
            rec(2, 90.0),
            rec(2, 90.0),
        ];
        let s = party_standings(&r, 2);
        assert_eq!(s[0].participant_id, 2);
        assert_eq!(
            (s[0].total, s[0].average, s[0].rounds, s[0].rank),
            (180.0, 90.0, 2, 1)
        );
        assert_eq!(
            (s[1].total, s[1].average, s[1].rounds, s[1].rank),
            (240.0, 80.0, 3, 2)
        );
        assert!(s.iter().all(|s| s.eligible));
    }

    // SPEC-008: minRounds 3 → B ineligible, listed below eligible ones.
    #[test]
    fn ineligible_ranks_below_eligible() {
        let r = [
            rec(1, 80.0),
            rec(1, 80.0),
            rec(1, 80.0),
            rec(2, 90.0),
            rec(2, 90.0),
        ];
        let s = party_standings(&r, 3);
        assert_eq!(s[0].participant_id, 1);
        assert!(s[0].eligible);
        assert_eq!(s[1].participant_id, 2);
        assert!(!s[1].eligible);
    }

    #[test]
    fn tie_breaks_by_total_then_id() {
        // Same average 70: participant 3 has more total; 1 and 2 fully tied → lower id first.
        let r = [rec(2, 70.0), rec(1, 70.0), rec(3, 70.0), rec(3, 70.0)];
        let ids: Vec<i64> = party_standings(&r, 1)
            .iter()
            .map(|s| s.participant_id)
            .collect();
        assert_eq!(ids, [3, 1, 2]);
    }

    #[test]
    fn standings_ignore_anonymous() {
        let r = [rec(1, 50.0), rec_at(None, 99.0, 0)];
        let s = party_standings(&r, 1);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].participant_id, 1);
    }

    // SPEC-008: same seed + inputs → same order.
    #[test]
    fn shuffle_is_deterministic() {
        let p = [1, 2, 3, 4, 5, 6];
        let s = songs(8);
        let a = plan_rounds(&p, &s, RoundOrder::Shuffled, 42);
        assert_eq!(a, plan_rounds(&p, &s, RoundOrder::Shuffled, 42));
        assert_ne!(a, plan_rounds(&p, &s, RoundOrder::Manual, 42));
        assert_ne!(a, plan_rounds(&p, &s, RoundOrder::Shuffled, 43));
    }

    #[test]
    fn plan_covers_everyone_and_every_song() {
        let p = [10, 20, 30];
        let s = songs(5);
        for order in [RoundOrder::Manual, RoundOrder::Shuffled] {
            let plan = plan_rounds(&p, &s, order, 7);
            assert_eq!(plan.len(), 5);
            assert!(p.iter().all(|id| plan.iter().any(|(pid, _)| pid == id)));
            assert!(s.iter().all(|song| plan.iter().any(|(_, ps)| ps == song)));
        }
        assert!(plan_rounds(&[], &s, RoundOrder::Manual, 0).is_empty());
        assert!(plan_rounds(&p, &[], RoundOrder::Shuffled, 0).is_empty());
    }

    #[test]
    fn manual_keeps_order() {
        let plan = plan_rounds(&[1, 2], &songs(2), RoundOrder::Manual, 0);
        assert_eq!(plan, [(1, "s0".to_string()), (2, "s1".to_string())]);
    }

    #[test]
    fn duel_picks_winner_or_draw() {
        let out = duel_outcome(&rec(1, 70.0), &rec(2, 85.5)).expect("compatible");
        assert_eq!(
            out,
            DuelOutcome {
                winner: Some(2),
                margin: 15.5
            }
        );
        let draw = duel_outcome(&rec(1, 60.0), &rec(2, 60.0)).expect("compatible");
        assert_eq!(draw.winner, None);
    }

    // Coordinator rule: scores equal after rounding to 0.1 are a draw; margin in tenths.
    #[test]
    fn duel_rounds_to_tenths() {
        let draw = duel_outcome(&rec(1, 80.04), &rec(2, 79.96)).expect("compatible");
        assert_eq!(
            draw,
            DuelOutcome {
                winner: None,
                margin: 0.0
            }
        );
        let close = duel_outcome(&rec(1, 80.06), &rec(2, 79.94)).expect("compatible");
        assert_eq!(
            close,
            DuelOutcome {
                winner: Some(1),
                margin: 0.2
            }
        );
    }

    #[test]
    fn duel_incompatible_errors() {
        let mut b = rec(2, 90.0);
        b.compat = key("hard");
        assert_eq!(
            duel_outcome(&rec(1, 80.0), &b),
            Err(GameError::IncompatibleRecords)
        );
    }

    #[test]
    fn challenge_progress_and_outcome() {
        let p = challenge_progress(&[80.0], 75.0, 3);
        assert_eq!(
            (p.completed, p.remaining, p.status),
            (1, 2, ChallengeStatus::InProgress)
        );
        let ok = challenge_progress(&[80.0, 70.0, 75.0], 75.0, 3);
        assert_eq!((ok.average, ok.status), (75.0, ChallengeStatus::Succeeded));
        let ko = challenge_progress(&[80.0, 60.0, 70.0, 100.0], 75.0, 3);
        assert_eq!(
            (ko.completed, ko.remaining, ko.status),
            (3, 0, ChallengeStatus::Failed)
        );
        assert_eq!(challenge_progress(&[], 50.0, 2).average, 0.0);
    }

    // SPEC-007 TEST
    #[test]
    fn leaderboard_excludes_incompatible() {
        let mut other_rev = rec(2, 99.0);
        other_rev.compat.reference_revision = 2;
        let mut other_algo = rec(3, 98.0);
        other_algo.compat.algorithm_version = 2;
        let mut other_diff = rec(4, 97.0);
        other_diff.compat = key("hard");
        let rows = leaderboard(
            &[rec(1, 50.0), other_rev, other_algo, other_diff],
            &key("normal"),
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].participant_id, Some(1));
    }

    #[test]
    fn ties_older_first() {
        let r = [
            rec_at(Some(1), 90.0, 300),
            rec_at(None, 90.0, 100),
            rec_at(Some(3), 95.0, 500),
        ];
        let rows = leaderboard(&r, &key("normal"));
        let order: Vec<_> = rows.iter().map(|r| (r.position, r.created_ms)).collect();
        assert_eq!(order, [(1, 500), (2, 100), (3, 300)]);
        assert_eq!(rows[1].participant_id, None);
    }

    #[test]
    fn top_50() {
        let r: Vec<_> = (0..60).map(|i| rec_at(Some(i), i as f32, i)).collect();
        let rows = leaderboard(&r, &key("normal"));
        assert_eq!(rows.len(), LEADERBOARD_SIZE);
        assert_eq!((rows[0].position, rows[0].score), (1, 59.0));
        assert_eq!(rows[49].score, 10.0);
    }

    #[test]
    fn ipc_types_are_camel_case() {
        let json = serde_json::to_string(&rec_at(None, 1.0, 2)).expect("serialize");
        assert!(json.contains("\"participantId\":null") && json.contains("\"createdMs\":2"));
        assert!(json.contains("\"referenceRevision\""));
    }
}
