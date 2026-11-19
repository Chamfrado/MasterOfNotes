//! SQLite access. Only Rust touches the database (Law 04).
//! Migrations are embedded and applied by `PRAGMA user_version`.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

/// Ordered migrations; index + 1 = schema version. Never edit an integrated entry.
pub const MIGRATIONS: &[&str] = &[include_str!("../migrations/001_init.sql")];

pub fn open(path: &Path) -> AppResult<Connection> {
    open_with(path, MIGRATIONS)
}

fn open_with(path: &Path, migrations: &[&str]) -> AppResult<Connection> {
    let mut conn = Connection::open(path).map_err(|e| open_failed(path, e))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&mut conn, path, migrations).map_err(|e| {
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
                "O banco em {} foi criado por uma versão mais nova do MasterOfNotes. Atualize o aplicativo.",
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
    }
}
