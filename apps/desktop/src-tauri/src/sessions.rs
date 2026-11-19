//! Participants, game sessions, rounds and leaderboards (SPEC-007 T-007-02, SPEC-008 T-008-02).
//! Ranking rules live in `crates/game`; this module only loads/stores.

use std::collections::HashSet;

use game::{CompatKey, GameMode, PerformanceRecord, RoundOrder, Standing};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::library::{self, lock};
use crate::settings::{self, Difficulty};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    pub id: i64,
    pub position: i64,
    pub song_id: String,
    pub participant_id: Option<i64>,
    pub status: String,
    /// Score of the latest competitive performance of this round.
    pub score: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_rounds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_score: Option<f32>,
    pub difficulty: Difficulty,
    /// < 2^53 so it survives a JS number.
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSession {
    pub id: i64,
    pub mode: GameMode,
    pub name: Option<String>,
    pub status: String,
    pub config: SessionConfig,
    pub started_ms: i64,
    pub completed_ms: Option<i64>,
    pub rounds: Vec<Round>,
    pub standings: Vec<Standing>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderArg {
    Manual,
    Shuffle,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSession {
    pub mode: GameMode,
    pub name: Option<String>,
    pub participant_ids: Vec<i64>,
    pub song_ids: Vec<String>,
    pub order: OrderArg,
    pub min_rounds: Option<u32>,
    pub target_score: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoundStatusArg {
    Pending,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardRow {
    pub rank: u32,
    pub participant_name: String,
    pub score: f32,
    pub created_ms: i64,
    pub performance_id: i64,
}

const ANONYMOUS: &str = "Anônimo";
const MAX_NAME: usize = 40;
const MAX_SESSION_SONGS: usize = 100;
const MAX_SESSION_PARTICIPANTS: usize = 20;

fn mode_str(m: GameMode) -> &'static str {
    match m {
        GameMode::Solo => "solo",
        GameMode::Party => "party",
        GameMode::Duel => "duel",
        GameMode::Challenge => "challenge",
    }
}

pub fn difficulty_str(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
}

fn clean_name(name: &str, what: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > MAX_NAME {
        return Err(AppError::invalid_input(format!(
            "O nome {what} deve ter entre 1 e {MAX_NAME} caracteres."
        )));
    }
    Ok(n.to_owned())
}

// ---------- Participants ----------

pub fn list_participants(conn: &Connection) -> AppResult<Vec<Participant>> {
    let mut stmt =
        conn.prepare("SELECT id, name FROM participants ORDER BY name COLLATE NOCASE, id")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Participant {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn create_participant(conn: &Connection, name: &str) -> AppResult<Participant> {
    let name = clean_name(name, "do participante")?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM participants WHERE name = ?1)",
        [&name],
        |r| r.get(0),
    )?;
    if exists {
        return Err(AppError::new(
            "participant_exists",
            format!("Já existe um participante chamado \"{name}\"."),
        ));
    }
    conn.execute(
        "INSERT INTO participants (name, created_ms) VALUES (?1, ?2)",
        (&name, now_ms()),
    )?;
    Ok(Participant {
        id: conn.last_insert_rowid(),
        name,
    })
}

// ---------- Sessions ----------

fn active_ref_status(conn: &Connection, song_id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT status FROM melody_references WHERE song_id = ?1 AND active = 1",
            [song_id],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn create_session(conn: &mut Connection, args: CreateSession) -> AppResult<GameSession> {
    let n_p = args.participant_ids.len();
    let n_s = args.song_ids.len();
    let rule = |ok: bool, msg: &str| {
        if ok {
            Ok(())
        } else {
            Err(AppError::invalid_input(msg))
        }
    };
    match args.mode {
        GameMode::Solo => rule(
            n_s == 1 && n_p <= 1,
            "O modo solo usa 1 música e no máximo 1 participante.",
        )?,
        GameMode::Party => rule(
            n_s >= 1 && n_p >= 2,
            "A festa precisa de pelo menos 2 participantes e 1 música.",
        )?,
        GameMode::Duel => rule(
            n_s == 1 && n_p == 2,
            "O duelo usa exatamente 2 participantes e 1 música.",
        )?,
        GameMode::Challenge => rule(
            n_s >= 1 && n_p <= 1,
            "O desafio precisa de pelo menos 1 música e no máximo 1 participante.",
        )?,
    }
    rule(
        n_s <= MAX_SESSION_SONGS && n_p <= MAX_SESSION_PARTICIPANTS,
        "Sessão grande demais.",
    )?;
    let unique: HashSet<_> = args.participant_ids.iter().collect();
    rule(unique.len() == n_p, "Participante repetido na sessão.")?;
    for &pid in &args.participant_ids {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM participants WHERE id = ?1)",
            [pid],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(AppError::new("not_found", "Participante não encontrado."));
        }
    }
    for song_id in &args.song_ids {
        let song = library::get_song(conn, song_id)?;
        if !song.available {
            return Err(AppError::invalid_input(format!(
                "O arquivo de \"{}\" não está disponível.",
                song.title
            )));
        }
        // Competitive modes need a validated reference (SPEC-008).
        if args.mode != GameMode::Solo
            && active_ref_status(conn, song_id)?.as_deref() != Some("validated")
        {
            return Err(AppError::new(
                "reference_not_validated",
                format!(
                    "\"{}\" ainda não tem referência melódica validada; ela é necessária para pontuação competitiva.",
                    song.title
                ),
            ));
        }
    }
    let name = match args
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        Some(n) => Some(clean_name(n, "da sessão")?),
        None => None,
    };
    if let Some(t) = args.target_score
        && !(0.0..=100.0).contains(&t)
    {
        return Err(AppError::invalid_input("A meta deve estar entre 0 e 100."));
    }
    if args.mode == GameMode::Challenge && args.target_score.is_none() {
        return Err(AppError::invalid_input(
            "O desafio precisa de uma meta de pontuação.",
        ));
    }
    if args.min_rounds == Some(0) {
        return Err(AppError::invalid_input(
            "O mínimo de rodadas deve ser pelo menos 1.",
        ));
    }

    let seed = (now_ms() as u64) & ((1 << 53) - 1);
    let order = match args.order {
        OrderArg::Manual => RoundOrder::Manual,
        OrderArg::Shuffle => RoundOrder::Shuffled,
    };
    let plan: Vec<(Option<i64>, String)> = if args.participant_ids.is_empty() {
        // Anonymous solo/challenge: one round per song.
        args.song_ids.iter().map(|s| (None, s.clone())).collect()
    } else {
        game::plan_rounds(&args.participant_ids, &args.song_ids, order, seed)
            .into_iter()
            .map(|(p, s)| (Some(p), s))
            .collect()
    };
    let min_rounds = match args.mode {
        // Default = fewest rounds assigned to any participant (SPEC-008).
        GameMode::Party => Some(args.min_rounds.unwrap_or_else(|| {
            args.participant_ids
                .iter()
                .map(|p| plan.iter().filter(|(q, _)| *q == Some(*p)).count() as u32)
                .min()
                .unwrap_or(1)
        })),
        _ => None,
    };
    let config = SessionConfig {
        min_rounds,
        target_score: args.target_score,
        difficulty: settings::load(conn)?.difficulty,
        seed,
    };

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO game_sessions (mode, name, status, config_json, started_ms) VALUES (?1, ?2, 'active', ?3, ?4)",
        (mode_str(args.mode), &name, serde_json::to_string(&config)?, now_ms()),
    )?;
    let id = tx.last_insert_rowid();
    for (i, (pid, song)) in plan.iter().enumerate() {
        tx.execute(
            "INSERT INTO rounds (session_id, position, song_id, participant_id, status) VALUES (?1, ?2, ?3, ?4, 'pending')",
            (id, i as i64 + 1, song, pid),
        )?;
    }
    tx.commit()?;
    get_session(conn, id)
}

pub fn get_session(conn: &Connection, id: i64) -> AppResult<GameSession> {
    let (mode, name, status, config_json, started_ms, completed_ms): (String, Option<String>, String, String, i64, Option<i64>) = conn
        .query_row(
            "SELECT mode, name, status, config_json, started_ms, completed_ms FROM game_sessions WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::new("not_found", "Sessão não encontrada."))?;
    let mode: GameMode = serde_json::from_value(serde_json::Value::String(mode))?;
    let config: SessionConfig = serde_json::from_str(&config_json)?;

    let mut stmt = conn.prepare(
        "SELECT r.id, r.position, r.song_id, r.participant_id, r.status,
                (SELECT p.score FROM performances p WHERE p.round_id = r.id AND p.competitive = 1 ORDER BY p.id DESC LIMIT 1)
         FROM rounds r WHERE r.session_id = ?1 ORDER BY r.position",
    )?;
    let rounds: Vec<Round> = stmt
        .query_map([id], |r| {
            Ok(Round {
                id: r.get(0)?,
                position: r.get(1)?,
                song_id: r.get(2)?,
                participant_id: r.get(3)?,
                status: r.get(4)?,
                score: r.get::<_, Option<f64>>(5)?.map(|s| s as f32),
            })
        })?
        .collect::<Result<_, _>>()?;

    // One record per completed round (its latest competitive performance).
    let records: Vec<PerformanceRecord> = rounds
        .iter()
        .filter(|r| r.status == "completed")
        .filter_map(|r| {
            Some(PerformanceRecord {
                participant_id: r.participant_id,
                song_id: r.song_id.clone(),
                score: r.score?,
                // Standings don't compare keys; every round of a session shares its rules.
                compat: CompatKey {
                    reference_id: 0,
                    reference_revision: 0,
                    algorithm_version: 0,
                    difficulty: String::new(),
                },
                created_ms: 0,
            })
        })
        .collect();
    let mut standings = game::party_standings(&records, config.min_rounds.unwrap_or(1));
    // Participants with no completed round yet are shown last, ineligible.
    let mut seen: HashSet<i64> = standings.iter().map(|s| s.participant_id).collect();
    for pid in rounds.iter().filter_map(|r| r.participant_id) {
        if seen.insert(pid) {
            standings.push(Standing {
                participant_id: pid,
                total: 0.0,
                average: 0.0,
                rounds: 0,
                eligible: false,
                rank: standings.len() as u32 + 1,
            });
        }
    }

    Ok(GameSession {
        id,
        mode,
        name,
        status,
        config,
        started_ms,
        completed_ms,
        rounds,
        standings,
    })
}

pub fn list_sessions(conn: &Connection) -> AppResult<Vec<GameSession>> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM game_sessions ORDER BY started_ms DESC, id DESC")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    ids.into_iter().map(|id| get_session(conn, id)).collect()
}

fn require_active(conn: &Connection, session_id: i64) -> AppResult<()> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM game_sessions WHERE id = ?1",
            [session_id],
            |r| r.get(0),
        )
        .optional()?;
    match status.as_deref() {
        None => Err(AppError::new("not_found", "Sessão não encontrada.")),
        Some("active") => Ok(()),
        Some(_) => Err(AppError::invalid_input("Esta sessão já foi encerrada.")),
    }
}

pub fn set_round_status(
    conn: &Connection,
    round_id: i64,
    status: RoundStatusArg,
) -> AppResult<GameSession> {
    let (session_id, current): (i64, String) = conn
        .query_row(
            "SELECT session_id, status FROM rounds WHERE id = ?1",
            [round_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::new("not_found", "Rodada não encontrada."))?;
    require_active(conn, session_id)?;
    if current == "completed" {
        return Err(AppError::invalid_input("Esta rodada já foi concluída."));
    }
    let s = match status {
        RoundStatusArg::Pending => "pending",
        RoundStatusArg::Skipped => "skipped",
    };
    conn.execute("UPDATE rounds SET status = ?2 WHERE id = ?1", (round_id, s))?;
    get_session(conn, session_id)
}

pub fn complete_session(conn: &Connection, id: i64) -> AppResult<GameSession> {
    require_active(conn, id)?;
    conn.execute(
        "UPDATE game_sessions SET status = 'completed', completed_ms = ?2 WHERE id = ?1",
        (id, now_ms()),
    )?;
    get_session(conn, id)
}

// ---------- Leaderboards ----------

/// Compatibility key of the song's active reference for `difficulty`; `None` without reference.
pub fn current_compat(
    conn: &Connection,
    song_id: &str,
    difficulty: &str,
) -> AppResult<Option<CompatKey>> {
    Ok(conn
        .query_row(
            "SELECT id, revision FROM melody_references WHERE song_id = ?1 AND active = 1",
            [song_id],
            |r| {
                Ok(CompatKey {
                    reference_id: r.get(0)?,
                    reference_revision: r.get(1)?,
                    algorithm_version: scoring_engine::ALGORITHM_VERSION,
                    difficulty: difficulty.to_owned(),
                })
            },
        )
        .optional()?)
}

/// Competitive performances of a song under `compat`, with ids and participant names.
pub fn competitive_records(
    conn: &Connection,
    song_id: &str,
    compat: &CompatKey,
) -> AppResult<Vec<(i64, Option<String>, PerformanceRecord)>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.participant_id, pa.name, p.score, p.created_ms FROM performances p
         LEFT JOIN participants pa ON pa.id = p.participant_id
         WHERE p.song_id = ?1 AND p.competitive = 1 AND p.reference_id = ?2 AND p.reference_revision = ?3
           AND p.algorithm_version = ?4 AND p.difficulty = ?5
         ORDER BY p.created_ms, p.id",
    )?;
    let rows = stmt
        .query_map(
            (
                song_id,
                compat.reference_id,
                compat.reference_revision,
                compat.algorithm_version,
                &compat.difficulty,
            ),
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(2)?,
                    PerformanceRecord {
                        participant_id: r.get(1)?,
                        song_id: song_id.to_owned(),
                        score: r.get::<_, f64>(3)? as f32,
                        compat: compat.clone(),
                        created_ms: r.get(4)?,
                    },
                ))
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn leaderboard(
    conn: &Connection,
    song_id: &str,
    difficulty: Difficulty,
) -> AppResult<Vec<LeaderboardRow>> {
    let Some(compat) = current_compat(conn, song_id, difficulty_str(difficulty))? else {
        return Ok(Vec::new());
    };
    let rows = competitive_records(conn, song_id, &compat)?;
    let records: Vec<PerformanceRecord> = rows.iter().map(|(_, _, r)| r.clone()).collect();
    // game::LeaderboardRow has no performance id: match each ranked row back to an unused
    // record with the same (participant, score, time). Identical tuples are interchangeable.
    let mut used = vec![false; rows.len()];
    Ok(game::leaderboard(&records, &compat)
        .into_iter()
        .filter_map(|row| {
            let i = rows.iter().enumerate().position(|(i, (_, _, r))| {
                !used[i]
                    && r.participant_id == row.participant_id
                    && r.score == row.score
                    && r.created_ms == row.created_ms
            })?;
            used[i] = true;
            let (perf_id, name, _) = &rows[i];
            Some(LeaderboardRow {
                rank: row.position,
                participant_name: name.clone().unwrap_or_else(|| ANONYMOUS.to_owned()),
                score: row.score,
                created_ms: row.created_ms,
                performance_id: *perf_id,
            })
        })
        .collect())
}

// ---------- Performance history (SPEC-007 R6, T-007-04) ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceRow {
    pub performance_id: i64,
    pub song_id: String,
    pub song_title: String,
    pub participant_name: Option<String>,
    pub score: f32,
    pub competitive: bool,
    pub algorithm_version: u32,
    pub reference_revision: i64,
    pub difficulty: String,
    pub created_ms: i64,
    /// Same CompatKey as the song has today (active reference + revision + current
    /// algorithm); `false` = "versão anterior".
    pub is_current_key: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListPerformances {
    pub song_id: Option<String>,
    pub participant_id: Option<i64>,
    pub limit: u32,
}

const MAX_HISTORY: u32 = 500;

/// Newest first; competitive and training; songs whose file vanished stay (history).
pub fn list_performances(
    conn: &Connection,
    args: &ListPerformances,
) -> AppResult<Vec<PerformanceRow>> {
    if !(1..=MAX_HISTORY).contains(&args.limit) {
        return Err(AppError::invalid_input(format!(
            "O limite deve estar entre 1 e {MAX_HISTORY}."
        )));
    }
    let mut stmt = conn.prepare(
        "SELECT p.id, p.song_id, s.title, pa.name, p.score, p.competitive, p.algorithm_version,
                COALESCE(p.reference_revision, 0), p.difficulty, p.created_ms,
                r.id IS NOT NULL AND r.id = p.reference_id AND r.revision = p.reference_revision
                    AND p.algorithm_version = ?3
         FROM performances p
         JOIN songs s ON s.id = p.song_id
         LEFT JOIN participants pa ON pa.id = p.participant_id
         LEFT JOIN melody_references r ON r.song_id = p.song_id AND r.active = 1
         WHERE (?1 IS NULL OR p.song_id = ?1) AND (?2 IS NULL OR p.participant_id = ?2)
         ORDER BY p.created_ms DESC, p.id DESC
         LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(
            (
                &args.song_id,
                args.participant_id,
                scoring_engine::ALGORITHM_VERSION,
                args.limit,
            ),
            |r| {
                Ok(PerformanceRow {
                    performance_id: r.get(0)?,
                    song_id: r.get(1)?,
                    song_title: r.get(2)?,
                    participant_name: r.get(3)?,
                    score: r.get::<_, f64>(4)? as f32,
                    competitive: r.get(5)?,
                    algorithm_version: r.get(6)?,
                    reference_revision: r.get(7)?,
                    difficulty: r.get(8)?,
                    created_ms: r.get(9)?,
                    is_current_key: r.get(10)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Tauri commands; names match ipc.ts.
pub mod cmd {
    use super::*;

    #[tauri::command]
    pub async fn list_participants(state: State<'_, AppState>) -> AppResult<Vec<Participant>> {
        super::list_participants(&lock(state.db()?))
    }

    #[tauri::command]
    pub async fn create_participant(
        state: State<'_, AppState>,
        name: String,
    ) -> AppResult<Participant> {
        super::create_participant(&lock(state.db()?), &name)
    }

    #[tauri::command]
    pub async fn create_session(
        state: State<'_, AppState>,
        args: CreateSession,
    ) -> AppResult<GameSession> {
        super::create_session(&mut lock(state.db()?), args)
    }

    #[tauri::command]
    pub async fn get_session(state: State<'_, AppState>, id: i64) -> AppResult<GameSession> {
        super::get_session(&lock(state.db()?), id)
    }

    #[tauri::command]
    pub async fn list_sessions(state: State<'_, AppState>) -> AppResult<Vec<GameSession>> {
        super::list_sessions(&lock(state.db()?))
    }

    #[tauri::command]
    pub async fn set_round_status(
        state: State<'_, AppState>,
        round_id: i64,
        status: RoundStatusArg,
    ) -> AppResult<GameSession> {
        super::set_round_status(&lock(state.db()?), round_id, status)
    }

    #[tauri::command]
    pub async fn complete_session(state: State<'_, AppState>, id: i64) -> AppResult<GameSession> {
        super::complete_session(&lock(state.db()?), id)
    }

    #[tauri::command]
    pub async fn get_leaderboard(
        state: State<'_, AppState>,
        song_id: String,
        difficulty: Difficulty,
    ) -> AppResult<Vec<LeaderboardRow>> {
        super::leaderboard(&lock(state.db()?), &song_id, difficulty)
    }

    #[tauri::command]
    pub async fn list_performances(
        state: State<'_, AppState>,
        args: ListPerformances,
    ) -> AppResult<Vec<PerformanceRow>> {
        super::list_performances(&lock(state.db()?), &args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Env {
        _dir: tempfile::TempDir,
        conn: Connection,
    }

    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        for (song, status) in [
            ("s1", Some("validated")),
            ("s2", Some("validated")),
            ("s3", Some("experimental")),
            ("s4", None),
        ] {
            conn.execute(
                "INSERT INTO songs (id, path, file_name, title, extension, size_bytes, modified_ms, added_ms, updated_ms)
                 VALUES (?1, ?2, ?3, ?1, 'mp4', 1, 1, 1, 1)",
                (song, format!("C:/v/{song}.mp4"), format!("{song}.mp4")),
            )
            .unwrap();
            if let Some(st) = status {
                conn.execute(
                    "INSERT INTO melody_references (song_id, source, format_version, status, notes_json, created_ms, updated_ms)
                     VALUES (?1, 'json', 1, ?2, '{}', 1, 1)",
                    (song, st),
                )
                .unwrap();
            }
        }
        Env { _dir: dir, conn }
    }

    fn args(mode: GameMode, participants: &[i64], songs: &[&str]) -> CreateSession {
        CreateSession {
            mode,
            name: Some(" Festa ".into()),
            participant_ids: participants.to_vec(),
            song_ids: songs.iter().map(|s| s.to_string()).collect(),
            order: OrderArg::Manual,
            min_rounds: None,
            target_score: None,
        }
    }

    /// Simulates finish_performance: competitive performance + round completed.
    fn sing(conn: &Connection, round: &Round, score: f64) {
        let (ref_id, rev): (i64, i64) = conn
            .query_row(
                "SELECT id, revision FROM melody_references WHERE song_id = ?1 AND active = 1",
                [&round.song_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO performances (round_id, song_id, participant_id, reference_id, reference_revision, algorithm_version,
             difficulty, competitive, score, metrics_json, capture_json, created_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'normal', 1, ?7, '{}', '{}', ?8)",
            (round.id, &round.song_id, round.participant_id, ref_id, rev, scoring_engine::ALGORITHM_VERSION, score, now_ms()),
        )
        .unwrap();
        conn.execute(
            "UPDATE rounds SET status = 'completed' WHERE id = ?1",
            [round.id],
        )
        .unwrap();
    }

    #[test]
    fn performance_history_filters_and_flags_old_keys() {
        let mut e = env();
        let ana = create_participant(&e.conn, "Ana").unwrap().id;
        let hist = |e: &Env, song: Option<&str>, participant: Option<i64>, limit: u32| {
            list_performances(
                &e.conn,
                &ListPerformances {
                    song_id: song.map(str::to_owned),
                    participant_id: participant,
                    limit,
                },
            )
        };
        let s1 = create_session(&mut e.conn, args(GameMode::Solo, &[ana], &["s1"])).unwrap();
        sing(&e.conn, &s1.rounds[0], 70.0);
        let anon = create_session(&mut e.conn, args(GameMode::Solo, &[], &["s1"])).unwrap();
        sing(&e.conn, &anon.rounds[0], 85.0);
        let s2 = create_session(&mut e.conn, args(GameMode::Solo, &[ana], &["s2"])).unwrap();
        sing(&e.conn, &s2.rounds[0], 60.0);

        let all = hist(&e, None, None, 50).unwrap();
        let view: Vec<_> = all
            .iter()
            .map(|r| (r.song_id.as_str(), r.participant_name.as_deref(), r.score))
            .collect();
        assert_eq!(
            view,
            [
                ("s2", Some("Ana"), 60.0),
                ("s1", None, 85.0),
                ("s1", Some("Ana"), 70.0)
            ],
            "newest first"
        );
        assert!(
            all.iter()
                .all(|r| r.is_current_key && r.competitive && r.reference_revision == 1)
        );
        assert_eq!(all[0].song_title, "s2");
        assert_eq!(hist(&e, Some("s1"), None, 50).unwrap().len(), 2);
        assert_eq!(hist(&e, None, Some(ana), 50).unwrap().len(), 2);
        assert_eq!(hist(&e, Some("s1"), Some(ana), 50).unwrap().len(), 1);
        assert_eq!(hist(&e, None, None, 1).unwrap()[0].song_id, "s2");
        assert!(hist(&e, Some("nope"), None, 50).unwrap().is_empty());

        // New reference revision → s1 rows become "versão anterior"; s2 untouched.
        e.conn
            .execute(
                "UPDATE melody_references SET revision = 2 WHERE song_id = 's1'",
                [],
            )
            .unwrap();
        let all = hist(&e, None, None, 50).unwrap();
        let current: Vec<_> = all
            .iter()
            .map(|r| (r.song_id.as_str(), r.is_current_key))
            .collect();
        assert_eq!(current, [("s2", true), ("s1", false), ("s1", false)]);
        // Song file gone: history stays.
        e.conn
            .execute("UPDATE songs SET available = 0 WHERE id = 's2'", [])
            .unwrap();
        assert_eq!(hist(&e, Some("s2"), None, 50).unwrap().len(), 1);

        for bad in [0, 501] {
            assert_eq!(hist(&e, None, None, bad).unwrap_err().code, "invalid_input");
        }
    }

    #[test]
    fn participants_unique_case_insensitive() {
        let e = env();
        let a = create_participant(&e.conn, "  Ana ").unwrap();
        assert_eq!(a.name, "Ana");
        assert_eq!(
            create_participant(&e.conn, "ANA").unwrap_err().code,
            "participant_exists"
        );
        assert_eq!(
            create_participant(&e.conn, "   ").unwrap_err().code,
            "invalid_input"
        );
        create_participant(&e.conn, "bia").unwrap();
        let names: Vec<_> = list_participants(&e.conn)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Ana", "bia"]);
    }

    #[test]
    fn mode_rules() {
        let mut e = env();
        let a = create_participant(&e.conn, "A").unwrap().id;
        let b = create_participant(&e.conn, "B").unwrap().id;
        let err = |e: &mut Env, x: CreateSession| create_session(&mut e.conn, x).unwrap_err().code;
        assert_eq!(
            err(&mut e, args(GameMode::Party, &[a], &["s1"])),
            "invalid_input"
        );
        assert_eq!(
            err(&mut e, args(GameMode::Party, &[a, a], &["s1"])),
            "invalid_input"
        );
        assert_eq!(
            err(&mut e, args(GameMode::Duel, &[a, b], &["s1", "s2"])),
            "invalid_input"
        );
        assert_eq!(
            err(&mut e, args(GameMode::Party, &[a, b], &["s3"])),
            "reference_not_validated"
        );
        assert_eq!(
            err(&mut e, args(GameMode::Party, &[a, b], &["s4"])),
            "reference_not_validated"
        );
        assert_eq!(
            err(&mut e, args(GameMode::Party, &[a, 999], &["s1"])),
            "not_found"
        );
        assert_eq!(
            err(&mut e, args(GameMode::Challenge, &[], &["s1"])),
            "invalid_input",
            "target required"
        );
        // Solo works without a reference and without a participant.
        let solo = create_session(&mut e.conn, args(GameMode::Solo, &[], &["s4"])).unwrap();
        assert_eq!(solo.rounds.len(), 1);
        assert_eq!(solo.rounds[0].participant_id, None);
        assert!(solo.standings.is_empty());
        let mut ch = args(GameMode::Challenge, &[], &["s1", "s2"]);
        ch.target_score = Some(70.0);
        let ch = create_session(&mut e.conn, ch).unwrap();
        assert_eq!((ch.rounds.len(), ch.config.target_score), (2, Some(70.0)));
    }

    #[test]
    fn party_standings_average_and_zero_round_participants() {
        let mut e = env();
        let a = create_participant(&e.conn, "A").unwrap().id;
        let b = create_participant(&e.conn, "B").unwrap().id;
        let c = create_participant(&e.conn, "C").unwrap().id;
        let s = create_session(
            &mut e.conn,
            args(GameMode::Party, &[a, b, c], &["s1", "s2"]),
        )
        .unwrap();
        assert_eq!(s.name.as_deref(), Some("Festa"));
        assert_eq!(s.rounds.len(), 3);
        assert_eq!(s.config.min_rounds, Some(1));
        assert_eq!(s.standings.len(), 3, "everyone listed before singing");
        assert!(s.standings.iter().all(|x| !x.eligible && x.rounds == 0));

        sing(&e.conn, &s.rounds[0], 80.0); // A
        sing(&e.conn, &s.rounds[1], 90.0); // B
        let s = get_session(&e.conn, s.id).unwrap();
        let order: Vec<_> = s
            .standings
            .iter()
            .map(|x| (x.participant_id, x.rank, x.eligible))
            .collect();
        assert_eq!(order, [(b, 1, true), (a, 2, true), (c, 3, false)]);
        assert_eq!(s.rounds[0].score, Some(80.0));

        let s = set_round_status(&e.conn, s.rounds[2].id, RoundStatusArg::Skipped).unwrap();
        assert_eq!(s.rounds[2].status, "skipped");
        assert_eq!(
            set_round_status(&e.conn, s.rounds[0].id, RoundStatusArg::Pending)
                .unwrap_err()
                .code,
            "invalid_input"
        );
        let done = complete_session(&e.conn, s.id).unwrap();
        assert_eq!(done.status, "completed");
        assert!(done.completed_ms.is_some());
        assert_eq!(
            complete_session(&e.conn, s.id).unwrap_err().code,
            "invalid_input"
        );
        assert_eq!(list_sessions(&e.conn).unwrap().len(), 1);
    }

    #[test]
    fn shuffle_is_reproducible_from_saved_seed() {
        let mut e = env();
        let ids: Vec<i64> = (0..5)
            .map(|i| create_participant(&e.conn, &format!("P{i}")).unwrap().id)
            .collect();
        let mut a = args(GameMode::Party, &ids, &["s1", "s2"]);
        a.order = OrderArg::Shuffle;
        let s = create_session(&mut e.conn, a).unwrap();
        let songs = vec!["s1".to_string(), "s2".to_string()];
        let replay = game::plan_rounds(&ids, &songs, RoundOrder::Shuffled, s.config.seed);
        let stored: Vec<_> = s
            .rounds
            .iter()
            .map(|r| (r.participant_id.unwrap(), r.song_id.clone()))
            .collect();
        assert_eq!(stored, replay);
        assert!(s.config.seed < 1 << 53);
    }

    #[test]
    fn leaderboard_current_key_only() {
        let mut e = env();
        let a = create_participant(&e.conn, "Ana").unwrap().id;
        let solo = create_session(&mut e.conn, args(GameMode::Solo, &[a], &["s1"])).unwrap();
        sing(&e.conn, &solo.rounds[0], 70.0);
        let anon = create_session(&mut e.conn, args(GameMode::Solo, &[], &["s1"])).unwrap();
        sing(&e.conn, &anon.rounds[0], 85.0);
        let rows = leaderboard(&e.conn, "s1", Difficulty::Normal).unwrap();
        let view: Vec<_> = rows
            .iter()
            .map(|r| (r.rank, r.participant_name.as_str(), r.score))
            .collect();
        assert_eq!(view, [(1, "Anônimo", 85.0), (2, "Ana", 70.0)]);
        assert_ne!(rows[0].performance_id, rows[1].performance_id);
        assert!(
            leaderboard(&e.conn, "s1", Difficulty::Hard)
                .unwrap()
                .is_empty(),
            "other difficulty"
        );
        // A new reference revision starts a fresh ranking.
        e.conn
            .execute(
                "UPDATE melody_references SET revision = 2 WHERE song_id = 's1'",
                [],
            )
            .unwrap();
        assert!(
            leaderboard(&e.conn, "s1", Difficulty::Normal)
                .unwrap()
                .is_empty()
        );
        assert!(
            leaderboard(&e.conn, "s4", Difficulty::Normal)
                .unwrap()
                .is_empty(),
            "no reference"
        );
    }
}
