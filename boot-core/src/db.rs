use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BootSession {
    pub id: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub total_duration_ms: Option<i64>,
    pub score: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProgramEvent {
    pub id: i64,
    pub session_id: i64,
    pub name: String,
    pub exe_path: Option<String>,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub status: String,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create db directory")?;
        }

        let conn = Connection::open(path).context("failed to open sqlite")?;
        conn.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA foreign_keys=ON;")?;

        let db = Database { conn };
        db.create_schema()?;
        Ok(db)
    }

    fn create_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS boot_session (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at        TEXT    NOT NULL,
                completed_at      TEXT,
                total_duration_ms INTEGER,
                score             INTEGER
            );

            CREATE TABLE IF NOT EXISTS program_event (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id  INTEGER NOT NULL REFERENCES boot_session(id),
                name        TEXT    NOT NULL,
                exe_path    TEXT,
                start_ms    INTEGER,
                end_ms      INTEGER,
                status      TEXT CHECK(status IN ('ok','slow','disabled','failed'))
            );

            CREATE INDEX IF NOT EXISTS idx_program_event_session
                ON program_event(session_id);
            ",
        )?;
        Ok(())
    }

    pub fn begin_session(&mut self) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO boot_session (started_at) VALUES (?1)",
            params![now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn complete_session(
        &mut self,
        session_id: i64,
        total_ms: i64,
        score: i64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE boot_session SET completed_at=?1, total_duration_ms=?2, score=?3 WHERE id=?4",
            params![now, total_ms, score, session_id],
        )?;
        Ok(())
    }

    pub fn upsert_program_event(
        &mut self,
        session_id: i64,
        name: &str,
        exe_path: Option<&str>,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        status: &str,
    ) -> Result<i64> {
        // 같은 세션 내 동일 exe_path가 있으면 UPDATE, 없으면 INSERT
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM program_event WHERE session_id=?1 AND name=?2",
                params![session_id, name],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            self.conn.execute(
                "UPDATE program_event SET end_ms=?1, status=?2 WHERE id=?3",
                params![end_ms, status, id],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO program_event (session_id, name, exe_path, start_ms, end_ms, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![session_id, name, exe_path, start_ms, end_ms, status],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn get_latest_session(&self) -> Result<Option<BootSession>> {
        let result = self.conn.query_row(
            "SELECT id, started_at, completed_at, total_duration_ms, score
             FROM boot_session ORDER BY id DESC LIMIT 1",
            [],
            |row| {
                Ok(BootSession {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    completed_at: row.get(2)?,
                    total_duration_ms: row.get(3)?,
                    score: row.get(4)?,
                })
            },
        );

        match result {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_program_events(&self, session_id: i64) -> Result<Vec<ProgramEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, name, exe_path, start_ms, end_ms, status
             FROM program_event WHERE session_id=?1 ORDER BY start_ms ASC",
        )?;

        let events = stmt
            .query_map(params![session_id], |row| {
                Ok(ProgramEvent {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    name: row.get(2)?,
                    exe_path: row.get(3)?,
                    start_ms: row.get(4)?,
                    end_ms: row.get(5)?,
                    status: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "ok".into()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(events)
    }

    pub fn get_recent_sessions(&self, limit: i64) -> Result<Vec<BootSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, started_at, completed_at, total_duration_ms, score
             FROM boot_session ORDER BY id DESC LIMIT ?1",
        )?;

        let sessions = stmt
            .query_map(params![limit], |row| {
                Ok(BootSession {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    completed_at: row.get(2)?,
                    total_duration_ms: row.get(3)?,
                    score: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(sessions)
    }
}

pub fn get_db_path() -> Result<PathBuf> {
    let appdata = std::env::var("APPDATA")
        .context("APPDATA env var not set")?;
    Ok(PathBuf::from(appdata).join("BootReady").join("data.db"))
}
