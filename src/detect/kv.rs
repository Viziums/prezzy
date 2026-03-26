use regex::Regex;
use std::sync::LazyLock;

use super::{Detector, Format};

/// Detects KEY=VALUE formatted output (env vars, config dumps, .env files).
///
/// Matches lines like `FOO=bar`, `DATABASE_URL=postgres://...`, `key = value`.
pub struct KeyValueDetector;

/// Matches `KEY=VALUE` with optional spaces around `=`.
/// Key must start with a letter or underscore, can contain alphanumeric, `_`, `.`, `-`.
static KV_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z_][\w.\-]*\s*=\s*\S").unwrap()
});

/// Lines that are just a key with empty value: `KEY=`
static KV_EMPTY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z_][\w.\-]*\s*=$").unwrap()
});

impl Detector for KeyValueDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.is_empty() {
            return 0.0;
        }

        let mut kv_count = 0;
        let mut total_non_empty = 0;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue; // Skip comments and blank lines.
            }
            total_non_empty += 1;

            if KV_LINE.is_match(trimmed) || KV_EMPTY.is_match(trimmed) {
                kv_count += 1;
            }
        }

        if total_non_empty < 2 {
            return 0.0;
        }

        let ratio = f64::from(kv_count) / f64::from(total_non_empty);

        if ratio >= 0.8 {
            0.82
        } else if ratio >= 0.6 {
            0.65
        } else {
            0.0
        }
    }

    fn format(&self) -> Format {
        Format::KeyValue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_env_vars() {
        let lines = vec![
            "HOME=/home/user".into(),
            "PATH=/usr/bin:/bin".into(),
            "SHELL=/bin/bash".into(),
            "TERM=xterm-256color".into(),
        ];
        assert!(KeyValueDetector.detect(&lines) > 0.8);
    }

    #[test]
    fn detects_dotenv() {
        let lines = vec![
            "# Database config".into(),
            "DATABASE_URL=postgres://localhost/app".into(),
            "REDIS_URL=redis://localhost:6379".into(),
            "SECRET_KEY=abc123".into(),
        ];
        assert!(KeyValueDetector.detect(&lines) > 0.8);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec!["hello world".into(), "not key value".into()];
        assert!(KeyValueDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn rejects_single_line() {
        let lines = vec!["FOO=bar".into()];
        assert!(KeyValueDetector.detect(&lines) < 0.1);
    }
}
