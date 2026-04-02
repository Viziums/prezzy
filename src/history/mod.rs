//! Command history for shell mode.
//!
//! Records every command executed in a prezzy shell session — command text,
//! duration, exit code, working directory, and detected output format — into
//! a local SQLite database.
//!
//! Privacy:
//! - Commands starting with a space are never recorded (bash convention).
//! - Set `PREZZY_NO_HISTORY=1` to disable recording entirely.
//! - `prezzy history --clear` wipes all data.

pub mod db;

pub use db::{CommandRecord, HistoryDb, HistoryStats, default_db_path, hostname, now_ms};

/// Returns `true` if history recording is disabled via environment variable.
pub fn is_disabled() -> bool {
    std::env::var_os("PREZZY_NO_HISTORY")
        .is_some_and(|v| !v.is_empty() && v != "0" && v != "false")
}

/// Returns `true` if the command should be skipped (space-prefixed).
pub fn should_skip(command: &str) -> bool {
    command.starts_with(' ')
}

/// Generate a session ID from PID + timestamp (unique per process).
pub fn session_id() -> String {
    let pid = std::process::id();
    let ts = now_ms();
    format!("{pid}-{ts}")
}
