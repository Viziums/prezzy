//! Command history for shell mode.
//!
//! Records every command executed in a prezzy shell session — command text,
//! duration, exit code, working directory, and detected output format — into
//! a local `SQLite` database.
//!
//! Privacy:
//! - Commands starting with a space are never recorded (bash convention).
//! - Set `PREZZY_NO_HISTORY=1` to disable recording entirely.
//! - `prezzy history --clear` wipes all data.

pub mod db;

pub use db::{CommandRecord, HistoryDb, HistoryStats, default_db_path, hostname, now_ms};

/// Returns `true` if history recording is disabled via environment variable.
#[must_use]
pub fn is_disabled() -> bool {
    std::env::var_os("PREZZY_NO_HISTORY").is_some_and(|v| !v.is_empty() && v != "0" && v != "false")
}

/// Returns `true` if the command should be skipped.
///
/// Skips space-prefixed commands (bash convention for secrets) and commands
/// matching any exclusion pattern from the config.
#[must_use]
pub fn should_skip(command: &str, exclude_patterns: &[String]) -> bool {
    if command.starts_with(' ') {
        return true;
    }
    let cmd_lower = command.to_lowercase();
    exclude_patterns
        .iter()
        .any(|pattern| glob_match(&cmd_lower, &pattern.to_lowercase()))
}

/// Simple glob matching: `*` matches any substring.
fn glob_match(text: &str, pattern: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return text == pattern;
    }

    // First part must match at the start.
    if !parts[0].is_empty() && !text.starts_with(parts[0]) {
        return false;
    }
    // Last part must match at the end.
    if let Some(last) = parts.last() {
        if !last.is_empty() && !text.ends_with(last) {
            return false;
        }
    }

    // Middle parts must appear in order.
    let mut pos = parts[0].len();
    for part in &parts[1..] {
        if part.is_empty() {
            continue;
        }
        if pos > text.len() {
            return false;
        }
        match text[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }
    true
}

/// Generate a session ID from PID + timestamp (unique per process).
#[must_use]
pub fn session_id() -> String {
    let pid = std::process::id();
    let ts = now_ms();
    format!("{pid}-{ts}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_space_prefixed() {
        assert!(should_skip(" secret command", &[]));
        assert!(!should_skip("normal command", &[]));
    }

    #[test]
    fn skip_excluded_patterns() {
        let patterns = vec!["*password*".into(), "*token*".into(), "*secret*".into()];
        assert!(should_skip("export PASSWORD=foo", &patterns));
        assert!(should_skip("curl -H 'token: abc'", &patterns));
        assert!(should_skip("echo secret_key", &patterns));
        assert!(!should_skip("echo hello", &patterns));
    }

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
    }

    #[test]
    fn glob_match_wildcard() {
        assert!(glob_match("hello world", "*world"));
        assert!(glob_match("hello world", "hello*"));
        assert!(glob_match("hello world", "*lo wo*"));
        assert!(glob_match("hello", "*"));
    }

    #[test]
    fn glob_match_multiple_wildcards() {
        assert!(glob_match("abcdefgh", "a*d*h"));
        assert!(!glob_match("abcdefgh", "a*z*h"));
    }

    #[test]
    fn glob_match_case_insensitive_via_caller() {
        // The caller lowercases both, so test that pathway.
        assert!(should_skip("export MY_TOKEN=abc", &["*token*".into()]));
    }

    #[test]
    fn glob_match_short_text_long_pattern() {
        // Pattern longer than text — must not panic.
        assert!(!glob_match("ab", "abcdefghij*"));
        assert!(!glob_match("x", "*very*long*pattern*"));
        assert!(!glob_match("", "*a*"));
    }

    #[test]
    fn glob_match_overlapping_parts() {
        assert!(glob_match("aab", "*a*b"));
        assert!(glob_match("aaab", "*a*a*b"));
        assert!(!glob_match("ab", "*a*a*b"));
    }

    #[test]
    fn glob_match_empty_text_and_pattern() {
        assert!(glob_match("", ""));
        assert!(glob_match("", "*"));
        assert!(!glob_match("", "a"));
    }
}
