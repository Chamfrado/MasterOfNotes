//! SQLite access. Only Rust touches the database (Law 04).
//! Migrations are embedded and applied by `PRAGMA user_version`.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

/// Ordered migrations; index + 1 = schema version. Never edit an integrated entry.
pub const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_ultrastar_source.sql"),
    include_str!("../migrations/003_reference_voice_level.sql"),
];

pub fn open(path: &Path) -> AppResult<Connection> {
    open_with(path, MIGRATIONS)
}

fn open_with(path: &Path, migrations: &[&str]) -> AppResult<Connection> {
    let mut conn = Connection::open(path).map_err(|e| open_failed(path, e))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    // Table rebuilds (SQLite's ALTER procedure) need FKs off; the pragma is a no-op inside a
    // transaction, so it is set around the migrations and each one runs foreign_key_check.
    conn.pragma_update(None, "foreign_keys", false)?;
    let migrated = migrate(&mut conn, path, migrations);
    conn.pragma_update(None, "foreign_keys", true)?;
    migrated.map_err(|e| {
        log::error!("migration failed for {}: {e}", path.display());
        e
    })?;
    Ok(conn)
}

fn migrate(conn: &mut Connection, path: &Path, migrations: &[&str]) -> AppResult<()> {
    let current = conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? as usize;
    let target = migrations.len();
    if current > target {
        return Err(AppError::new(
            "db_too_new",
            format!(
                "O banco em {} foi criado por uma versão mais nova do Master Of Notes. Atualize o aplicativo.",
                path.display()
            ),
        ));
    }
    if current == target {
        return Ok(());
    }
    if current > 0 {
        // Existing data: back it up before touching the schema (Law 14).
        // VACUUM INTO copies a consistent snapshot, including pages still in the WAL.
        let backup = path.with_file_name(format!(
            "{}.bak-v{current}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("masterofnotes.db")
        ));
        if backup.exists() {
            std::fs::remove_file(&backup)?; // stale backup of the same version
        }
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])?;
        log::info!("database backup written to {}", backup.display());
    }
    for (i, sql) in migrations.iter().enumerate().skip(current) {
        let version = i + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        // Any dangling reference → the transaction rolls back on drop, data untouched.
        let violations: i64 =
            tx.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })?;
        if violations > 0 {
            return Err(AppError::new(
                "db_migration_failed",
                format!(
                    "A atualização do banco para a versão {version} encontrou {violations} referência(s) inconsistente(s) e foi desfeita. Nenhum dado foi alterado."
                ),
            ));
        }
        tx.pragma_update(None, "user_version", version as i64)?;
        tx.commit()?;
        log::info!("database migrated to v{version}");
    }
    Ok(())
}

fn open_failed(path: &Path, e: rusqlite::Error) -> AppError {
    log::error!("cannot open database {}: {e}", path.display());
    AppError::new(
        "db_open_failed",
        format!(
            "Não foi possível abrir o banco de dados em {}. Nenhum dado foi apagado.",
            path.display()
        ),
    )
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(conn: &Connection) -> usize {
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap() as usize
    }

    #[test]
    fn migrates_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("masterofnotes.db");
        let conn = open(&path).unwrap();
        assert_eq!(version(&conn), MIGRATIONS.len());
        let fk: i64 = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        for table in [
            "settings",
            "songs",
            "melody_references",
            "participants",
            "game_sessions",
            "rounds",
            "performances",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "missing table {table}");
        }
        // Fresh database: nothing to back up.
        assert!(!dir.path().join("masterofnotes.db.bak-v0").exists());
    }

    #[test]
    fn reopen_preserves_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("masterofnotes.db");
        {
            let conn = open(&path).unwrap();
            conn.execute(
                "INSERT INTO participants (name, created_ms) VALUES ('Ana', 1)",
                [],
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        let name: String = conn
            .query_row("SELECT name FROM participants", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Ana");
        assert_eq!(version(&conn), MIGRATIONS.len());
    }

    #[test]
    fn backup_before_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("masterofnotes.db");
        {
            let conn = open_with(&path, &MIGRATIONS[..1]).unwrap();
            conn.execute(
                "INSERT INTO participants (name, created_ms) VALUES ('Ana', 1)",
                [],
            )
            .unwrap();
        }
        let next = [
            MIGRATIONS[0],
            "CREATE TABLE extra (id INTEGER PRIMARY KEY);",
        ];
        let conn = open_with(&path, &next).unwrap();
        assert_eq!(version(&conn), 2);
        let n: i64 = conn
            .query_row("SELECT count(*) FROM participants", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "data kept after migration");

        let backup = dir.path().join("masterofnotes.db.bak-v1");
        let bak = Connection::open(&backup).unwrap();
        assert_eq!(version(&bak), 1);
        let name: String = bak
            .query_row("SELECT name FROM participants", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Ana");
    }

    /// Every row of every user table, as text (order by rowid/primary key).
    fn dump(conn: &Connection) -> Vec<String> {
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut out = Vec::new();
        for t in tables {
            let mut stmt = conn
                .prepare(&format!("SELECT * FROM {t} ORDER BY 1"))
                .unwrap();
            let n = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..n)
                        .map(|i| r.get::<_, rusqlite::types::Value>(i))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap();
            for row in rows {
                out.push(format!("{t}: {:?}", row.unwrap()));
            }
        }
        out
    }

    /// A database exactly as 0.1.3 left it (schema v1) with real-looking data.
    fn v013_database(path: &Path) {
        let conn = open_with(path, &MIGRATIONS[..1]).unwrap();
        conn.execute_batch(
            r#"
            INSERT INTO settings VALUES ('videoVolume', '0.8'), ('leakCheck', '{"deviceId":null,"videoVolume":0.8,"passed":true,"atMs":5}');
            INSERT INTO songs (id, path, file_name, title, artist, extension, size_bytes, modified_ms, duration_ms, playable, available, added_ms, updated_ms) VALUES
              ('a1', 'C:/v/a.mp4', 'a.mp4', 'Canção A', 'Banda', 'mp4', 100, 1, 200000, 'yes', 1, 1, 2),
              ('b2', 'C:/v/b.mkv', 'b.mkv', 'b', NULL, 'mkv', 200, 1, NULL, 'unknown', 0, 1, 1);
            INSERT INTO melody_references (id, song_id, source, format_version, revision, status, offset_ms, notes_json, active, created_ms, updated_ms) VALUES
              (1, 'a1', 'midi', 1, 1, 'validated', 0, '{"format":"mon-melody","version":1,"notes":[{"startMs":0,"durMs":500,"midi":60}]}', 0, 10, 11),
              (2, 'a1', 'json', 1, 3, 'validated', 120, '{"format":"mon-melody","version":1,"notes":[{"startMs":100,"durMs":400,"midi":62,"lyric":"lá"}]}', 1, 20, 25),
              (5, 'b2', 'extracted', 1, 1, 'experimental', -40, '{"format":"mon-melody","version":1,"notes":[{"startMs":0,"durMs":300,"midi":64}]}', 1, 30, 30);
            INSERT INTO participants VALUES (1, 'Ana', 1), (2, 'Bia', 2);
            INSERT INTO game_sessions VALUES (1, 'party', 'Festa', 'completed', '{"minRounds":2,"seed":42}', 100, 200);
            INSERT INTO rounds VALUES (1, 1, 0, 'a1', 1, 'completed'), (2, 1, 1, 'a1', 2, 'skipped');
            INSERT INTO performances (id, round_id, song_id, participant_id, reference_id, reference_revision, algorithm_version, difficulty, competitive, score, metrics_json, capture_json, created_ms) VALUES
              (1, NULL, 'a1', NULL, 1, 1, 1, 'normal', 1, 71.5, '{}', '{}', 50),
              (2, 1, 'a1', 1, 2, 3, 1, 'normal', 1, 88.25, '{}', '{}', 150),
              (3, NULL, 'b2', 2, 5, 1, 1, 'hard', 0, 40.0, '{}', '{}', 160);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn migration_002_keeps_013_data_and_accepts_ultrastar() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("masterofnotes.db");
        v013_database(&path);
        let before = dump(&Connection::open(&path).unwrap());
        assert_eq!(before.len(), 15, "fixture rows");

        let conn = open_with(&path, &MIGRATIONS[..2]).unwrap();
        assert_eq!(version(&conn), 2);
        assert_eq!(
            dump(&conn),
            before,
            "every row of every table preserved, ids included"
        );
        let fk_errors: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(fk_errors, 0);
        let fk_on: i64 = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        assert_eq!(fk_on, 1, "FKs back on after migrating");

        // Backup of the 0.1.3 database, identical data.
        let bak = Connection::open(dir.path().join("masterofnotes.db.bak-v1")).unwrap();
        assert_eq!(version(&bak), 1);
        assert_eq!(dump(&bak), before);

        // New schema: 'ultrastar' accepted, the rest of the CHECK, the partial unique index
        // and the FKs from/to melody_references still enforced.
        let insert = |source: &str, active: i64| {
            conn.execute(
                "INSERT INTO melody_references (song_id, source, format_version, status, notes_json, active, created_ms, updated_ms)
                 VALUES ('b2', ?1, 1, 'experimental', '{}', ?2, 1, 1)",
                (source, active),
            )
        };
        assert!(insert("bogus", 0).is_err(), "CHECK kept");
        assert!(insert("ultrastar", 1).is_err(), "one_active_reference kept");
        insert("ultrastar", 0).unwrap();
        assert!(
            conn.execute(
                "INSERT INTO melody_references (song_id, source, format_version, status, notes_json, created_ms, updated_ms)
                 VALUES ('zz', 'json', 1, 'experimental', '{}', 1, 1)",
                [],
            )
            .is_err(),
            "FK to songs kept"
        );
        assert!(
            conn.execute(
                "INSERT INTO performances (song_id, reference_id, algorithm_version, difficulty, competitive, score, metrics_json, capture_json, created_ms)
                 VALUES ('a1', 999, 1, 'normal', 0, 1, '{}', '{}', 1)",
                [],
            )
            .is_err(),
            "FK from performances to the rebuilt table kept"
        );
        let schema: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'performances'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(schema.contains("REFERENCES melody_references(id)"));

        // Reopening a v2 database: no new backup, nothing changes.
        drop((conn, bak));
        std::fs::remove_file(dir.path().join("masterofnotes.db.bak-v1")).unwrap();
        let conn = open_with(&path, &MIGRATIONS[..2]).unwrap();
        assert_eq!(version(&conn), 2);
        assert!(!dir.path().join("masterofnotes.db.bak-v2").exists());
    }

    #[test]
    fn migration_003_adds_a_null_voice_level_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("masterofnotes.db");
        v013_database(&path);
        drop(open_with(&path, &MIGRATIONS[..2]).unwrap());
        let before = dump(&Connection::open(&path).unwrap());

        let conn = open(&path).unwrap();
        assert_eq!(version(&conn), 3);
        // Same rows; references gain one trailing NULL column.
        let expected: Vec<String> = before
            .iter()
            .map(|l| match l.strip_prefix("melody_references: ") {
                Some(row) => format!("melody_references: {}, Null]", &row[..row.len() - 1]),
                None => l.clone(),
            })
            .collect();
        assert_eq!(dump(&conn), expected);
        assert!(dir.path().join("masterofnotes.db.bak-v2").exists());
    }

    #[test]
    fn migration_with_dangling_reference_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("masterofnotes.db");
        v013_database(&path);
        let before = dump(&Connection::open(&path).unwrap());
        let bad = [MIGRATIONS[0], "DELETE FROM melody_references WHERE id = 2;"];
        assert_eq!(
            open_with(&path, &bad).unwrap_err().code,
            "db_migration_failed"
        );
        let conn = Connection::open(&path).unwrap();
        assert_eq!(version(&conn), 1, "not migrated");
        assert_eq!(dump(&conn), before, "rolled back");
    }

    #[test]
    fn refuses_newer_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("masterofnotes.db");
        {
            let conn = open(&path).unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        let err = open(&path).unwrap_err();
        assert_eq!(err.code, "db_too_new");
        assert!(err.message.contains("versão mais nova do Master Of Notes."));
    }
}
