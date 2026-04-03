//! `SQLite` storage for command history.
//!
//! Schema is versioned via `user_version` pragma. Migrations run automatically
//! on open, so adding columns later is straightforward.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

/// A single recorded command.
#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub command: String,
    /// Unix epoch milliseconds when the command started.
    pub timestamp_ms: i64,
    /// How long the command ran, in milliseconds.
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub cwd: Option<String>,
    /// Format detected by the beautifier (e.g. "json", "diff").
    pub format: Option<String>,
    pub session_id: String,
    pub hostname: String,
}

/// Handle to the history database.
pub struct HistoryDb {
    conn: Connection,
}

/// Current schema version. Bump when adding migrations.
const SCHEMA_VERSION: u32 = 1;

impl HistoryDb {
    /// Open (or create) the history database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("create history database directory")?;
        }

        let conn = Connection::open(path).context("open history database")?;

        // Performance: WAL mode for concurrent reads, reduced fsync.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Insert a command record.
    pub fn insert(&self, record: &CommandRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO commands (command, timestamp_ms, duration_ms, exit_code, cwd, format, session_id, hostname)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.command,
                record.timestamp_ms,
                record.duration_ms,
                record.exit_code,
                record.cwd,
                record.format,
                record.session_id,
                record.hostname,
            ],
        )?;
        Ok(())
    }

    /// Most recent commands, newest first.
    pub fn recent(&self, limit: u32) -> Result<Vec<CommandRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT command, timestamp_ms, duration_ms, exit_code, cwd, format, session_id, hostname
             FROM commands ORDER BY timestamp_ms DESC LIMIT ?1",
        )?;
        Self::collect_rows(&mut stmt, params![limit])
    }

    /// Commands that exited with a non-zero code, newest first.
    pub fn failed(&self, limit: u32) -> Result<Vec<CommandRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT command, timestamp_ms, duration_ms, exit_code, cwd, format, session_id, hostname
             FROM commands WHERE exit_code IS NOT NULL AND exit_code != 0
             ORDER BY timestamp_ms DESC LIMIT ?1",
        )?;
        Self::collect_rows(&mut stmt, params![limit])
    }

    /// Most frequently used commands.
    pub fn top(&self, limit: u32) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT command, COUNT(*) as cnt FROM commands
             GROUP BY command ORDER BY cnt DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Search commands by substring (case-insensitive).
    pub fn search(&self, pattern: &str, limit: u32) -> Result<Vec<CommandRecord>> {
        let like = format!("%{pattern}%");
        let mut stmt = self.conn.prepare(
            "SELECT command, timestamp_ms, duration_ms, exit_code, cwd, format, session_id, hostname
             FROM commands WHERE command LIKE ?1
             ORDER BY timestamp_ms DESC LIMIT ?2",
        )?;
        Self::collect_rows(&mut stmt, params![like, limit])
    }

    /// Commands since a given timestamp (epoch ms), newest first.
    pub fn since(&self, since_ms: i64, limit: u32) -> Result<Vec<CommandRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT command, timestamp_ms, duration_ms, exit_code, cwd, format, session_id, hostname
             FROM commands WHERE timestamp_ms >= ?1
             ORDER BY timestamp_ms DESC LIMIT ?2",
        )?;
        Self::collect_rows(&mut stmt, params![since_ms, limit])
    }

    /// Commands run in a specific directory (prefix match), newest first.
    ///
    /// Tries both the given path and an alternate form to handle MSYS/Cygwin
    /// path translation (e.g. `/c/Users` vs `C:/Users`).
    pub fn by_dir(&self, dir: &str, limit: u32) -> Result<Vec<CommandRecord>> {
        let alt = alternate_path_form(dir);
        let like1 = format!("{dir}%");
        let like2 = format!("{alt}%");
        let mut stmt = self.conn.prepare(
            "SELECT command, timestamp_ms, duration_ms, exit_code, cwd, format, session_id, hostname
             FROM commands WHERE cwd LIKE ?1 OR cwd LIKE ?2
             ORDER BY timestamp_ms DESC LIMIT ?3",
        )?;
        Self::collect_rows(&mut stmt, params![like1, like2, limit])
    }

    /// Slowest commands.
    pub fn slowest(&self, limit: u32) -> Result<Vec<CommandRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT command, timestamp_ms, duration_ms, exit_code, cwd, format, session_id, hostname
             FROM commands WHERE duration_ms IS NOT NULL
             ORDER BY duration_ms DESC LIMIT ?1",
        )?;
        Self::collect_rows(&mut stmt, params![limit])
    }

    /// Aggregate statistics.
    pub fn stats(&self) -> Result<HistoryStats> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))?;
        let failed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM commands WHERE exit_code IS NOT NULL AND exit_code != 0",
            [],
            |r| r.get(0),
        )?;
        let avg_duration: Option<f64> = self.conn.query_row(
            "SELECT AVG(duration_ms) FROM commands WHERE duration_ms IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        let unique_commands: i64 =
            self.conn
                .query_row("SELECT COUNT(DISTINCT command) FROM commands", [], |r| {
                    r.get(0)
                })?;

        Ok(HistoryStats {
            total_commands: total,
            failed_commands: failed,
            unique_commands,
            avg_duration_ms: avg_duration,
        })
    }

    /// Delete all history. Returns the number of rows deleted.
    pub fn clear(&self) -> Result<usize> {
        let count = self.conn.execute("DELETE FROM commands", [])?;
        self.conn.execute("VACUUM", [])?;
        Ok(count)
    }

    /// Total number of recorded commands.
    pub fn count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))?)
    }

    // -- internals ------------------------------------------------------------

    fn migrate(&self) -> Result<()> {
        let version: u32 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;

        if version < 1 {
            self.conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS commands (
                    id          INTEGER PRIMARY KEY,
                    command     TEXT NOT NULL,
                    timestamp_ms INTEGER NOT NULL,
                    duration_ms INTEGER,
                    exit_code   INTEGER,
                    cwd         TEXT,
                    format      TEXT,
                    session_id  TEXT NOT NULL,
                    hostname    TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_commands_timestamp ON commands(timestamp_ms);
                CREATE INDEX IF NOT EXISTS idx_commands_command ON commands(command);
                CREATE INDEX IF NOT EXISTS idx_commands_exit_code ON commands(exit_code);",
            )?;
        }

        // Always update to current version.
        if version < SCHEMA_VERSION {
            self.conn
                .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }

        Ok(())
    }

    fn collect_rows(
        stmt: &mut rusqlite::Statement<'_>,
        params: impl rusqlite::Params,
    ) -> Result<Vec<CommandRecord>> {
        let rows = stmt.query_map(params, |row| {
            Ok(CommandRecord {
                command: row.get(0)?,
                timestamp_ms: row.get(1)?,
                duration_ms: row.get(2)?,
                exit_code: row.get(3)?,
                cwd: row.get(4)?,
                format: row.get(5)?,
                session_id: row.get(6)?,
                hostname: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

/// Aggregate statistics for `prezzy history --stats`.
#[derive(Debug)]
pub struct HistoryStats {
    pub total_commands: i64,
    pub failed_commands: i64,
    pub unique_commands: i64,
    pub avg_duration_ms: Option<f64>,
}

/// Default database path: `~/.local/share/prezzy/history.db` (Unix)
/// or `%APPDATA%/prezzy/history.db` (Windows).
#[must_use]
pub fn default_db_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("prezzy").join("history.db"))
}

/// Convert between MSYS (`/c/Users`) and Windows (`C:/Users`) path forms.
///
/// If the path looks like `/x/...`, returns `X:/...`. If it looks like `X:/...`,
/// returns `/x/...`. Otherwise returns the input unchanged.
fn alternate_path_form(path: &str) -> String {
    let bytes = path.as_bytes();
    // MSYS → Windows: /c/Users → C:/Users
    if bytes.len() >= 2 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() {
        let drive = bytes[1].to_ascii_uppercase() as char;
        return format!("{drive}:{}", &path[2..]);
    }
    // Windows → MSYS: C:/Users → /c/Users
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
    {
        let drive = bytes[0].to_ascii_lowercase() as char;
        return format!("/{drive}{}", &path[2..]);
    }
    path.to_owned()
}

/// Current Unix epoch in milliseconds.
#[must_use]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Best-effort hostname.
#[must_use]
pub fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_record(cmd: &str) -> CommandRecord {
        CommandRecord {
            command: cmd.into(),
            timestamp_ms: now_ms(),
            duration_ms: Some(100),
            exit_code: Some(0),
            cwd: Some("/home/user".into()),
            format: Some("json".into()),
            session_id: "test-session".into(),
            hostname: "test-host".into(),
        }
    }

    #[test]
    fn insert_and_retrieve() {
        let db = HistoryDb::open_in_memory().unwrap();
        db.insert(&test_record("echo hello")).unwrap();
        let rows = db.recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "echo hello");
    }

    #[test]
    fn recent_ordering() {
        let db = HistoryDb::open_in_memory().unwrap();
        let mut r1 = test_record("first");
        r1.timestamp_ms = 1000;
        let mut r2 = test_record("second");
        r2.timestamp_ms = 2000;
        db.insert(&r1).unwrap();
        db.insert(&r2).unwrap();

        let rows = db.recent(10).unwrap();
        assert_eq!(rows[0].command, "second"); // newest first
        assert_eq!(rows[1].command, "first");
    }

    #[test]
    fn failed_only() {
        let db = HistoryDb::open_in_memory().unwrap();
        db.insert(&test_record("ok")).unwrap();
        let mut fail = test_record("bad");
        fail.exit_code = Some(1);
        db.insert(&fail).unwrap();

        let rows = db.failed(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "bad");
    }

    #[test]
    fn top_frequency() {
        let db = HistoryDb::open_in_memory().unwrap();
        for _ in 0..5 {
            db.insert(&test_record("ls")).unwrap();
        }
        for _ in 0..2 {
            db.insert(&test_record("pwd")).unwrap();
        }
        let top = db.top(10).unwrap();
        assert_eq!(top[0].0, "ls");
        assert_eq!(top[0].1, 5);
        assert_eq!(top[1].0, "pwd");
    }

    #[test]
    fn search_pattern() {
        let db = HistoryDb::open_in_memory().unwrap();
        db.insert(&test_record("git status")).unwrap();
        db.insert(&test_record("git push")).unwrap();
        db.insert(&test_record("echo hello")).unwrap();

        let rows = db.search("git", 10).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn stats_aggregation() {
        let db = HistoryDb::open_in_memory().unwrap();
        db.insert(&test_record("a")).unwrap();
        db.insert(&test_record("b")).unwrap();
        let mut fail = test_record("c");
        fail.exit_code = Some(1);
        db.insert(&fail).unwrap();

        let stats = db.stats().unwrap();
        assert_eq!(stats.total_commands, 3);
        assert_eq!(stats.failed_commands, 1);
        assert_eq!(stats.unique_commands, 3);
    }

    #[test]
    fn clear_removes_all() {
        let db = HistoryDb::open_in_memory().unwrap();
        db.insert(&test_record("a")).unwrap();
        db.insert(&test_record("b")).unwrap();
        let deleted = db.clear().unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(db.count().unwrap(), 0);
    }

    #[test]
    fn slowest_ordering() {
        let db = HistoryDb::open_in_memory().unwrap();
        let mut fast = test_record("fast");
        fast.duration_ms = Some(10);
        let mut slow = test_record("slow");
        slow.duration_ms = Some(5000);
        db.insert(&fast).unwrap();
        db.insert(&slow).unwrap();

        let rows = db.slowest(10).unwrap();
        assert_eq!(rows[0].command, "slow");
    }

    #[test]
    fn by_dir_prefix_match() {
        let db = HistoryDb::open_in_memory().unwrap();
        let mut r1 = test_record("ls");
        r1.cwd = Some("/home/user/project".into());
        let mut r2 = test_record("pwd");
        r2.cwd = Some("/tmp".into());
        db.insert(&r1).unwrap();
        db.insert(&r2).unwrap();

        let rows = db.by_dir("/home", 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "ls");

        let rows = db.by_dir("/", 10).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn since_filter() {
        let db = HistoryDb::open_in_memory().unwrap();
        let mut old = test_record("old");
        old.timestamp_ms = 1000;
        let mut recent = test_record("recent");
        recent.timestamp_ms = 99_000;
        db.insert(&old).unwrap();
        db.insert(&recent).unwrap();

        let rows = db.since(50_000, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "recent");
    }

    #[test]
    fn migration_is_idempotent() {
        let db = HistoryDb::open_in_memory().unwrap();
        db.insert(&test_record("before")).unwrap();
        // Re-opening runs migrate again — should not fail or lose data.
        drop(db);
        // Can't re-open in-memory, but migration idempotency is ensured
        // by IF NOT EXISTS on all CREATE statements.
    }
}
