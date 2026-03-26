use regex::Regex;
use std::sync::LazyLock;

use super::{Detector, Format};

/// Detects plain-text log lines.
///
/// Looks for lines matching common patterns:
///   - Timestamp prefix + optional level keyword
///   - Level keyword at start of line
///   - Common log framework output formats
pub struct LogDetector;

/// Matches common timestamp patterns at the start of a line:
///   2024-01-15T10:30:45Z
///   2024-01-15 10:30:45,123
///   Jan 15 10:30:45
///   [2024-01-15 10:30:45]
///   10:30:45.123
static TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^[\[\s]*                           # optional bracket/whitespace
        (?:
            \d{4}[-/]\d{2}[-/]\d{2}       # ISO date: 2024-01-15
            [T\s]\d{2}:\d{2}:\d{2}        # Time: T10:30:45
            |
            [A-Z][a-z]{2}\s+\d{1,2}\s+    # Syslog: Jan 15
            \d{2}:\d{2}:\d{2}             # Time: 10:30:45
            |
            \d{2}:\d{2}:\d{2}             # Time only: 10:30:45
        )"
    ).unwrap()
});

/// Matches log level keywords (case insensitive).
static LEVEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(ERROR|WARN(?:ING)?|INFO|DEBUG|TRACE|FATAL|CRITICAL|CRIT|ERR|DBG|TRC|VERBOSE)\b").unwrap()
});

impl Detector for LogDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.is_empty() {
            return 0.0;
        }

        let mut timestamp_count = 0;
        let mut level_count = 0;
        let mut total_non_empty = 0;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            total_non_empty += 1;

            if TIMESTAMP_RE.is_match(trimmed) {
                timestamp_count += 1;
            }
            if LEVEL_RE.is_match(trimmed) {
                level_count += 1;
            }
        }

        if total_non_empty == 0 {
            return 0.0;
        }

        let ts_ratio = f64::from(timestamp_count) / f64::from(total_non_empty);
        let level_ratio = f64::from(level_count) / f64::from(total_non_empty);

        // Strong signal: most lines have both timestamp and level.
        if ts_ratio >= 0.7 && level_ratio >= 0.5 {
            return 0.88;
        }

        // Moderate signal: most lines have timestamps OR levels.
        if ts_ratio >= 0.6 || level_ratio >= 0.6 {
            return 0.7;
        }

        // Weak signal: some lines have timestamps and levels.
        if ts_ratio >= 0.3 && level_ratio >= 0.3 {
            return 0.55;
        }

        0.0
    }

    fn format(&self) -> Format {
        Format::Log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_iso_timestamp_logs() {
        let lines = vec![
            "2024-01-15T10:30:45.123Z ERROR Failed to connect".into(),
            "2024-01-15T10:30:46.456Z INFO  Retrying connection".into(),
            "2024-01-15T10:30:47.789Z WARN  Slow query detected".into(),
        ];
        assert!(LogDetector.detect(&lines) > 0.8);
    }

    #[test]
    fn detects_syslog_format() {
        let lines = vec![
            "Jan 15 10:30:45 server sshd[1234]: Accepted publickey".into(),
            "Jan 15 10:30:46 server sshd[1234]: session opened".into(),
            "Jan 15 10:30:47 server nginx[5678]: GET /api 200".into(),
        ];
        // These have timestamps but no explicit level keywords.
        let score = LogDetector.detect(&lines);
        assert!(score > 0.5);
    }

    #[test]
    fn detects_level_only_logs() {
        let lines = vec![
            "ERROR: connection refused".into(),
            "WARN: retrying in 5s".into(),
            "INFO: connected successfully".into(),
        ];
        assert!(LogDetector.detect(&lines) > 0.6);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec![
            "hello world".into(),
            "this is just text".into(),
            "nothing to see here".into(),
        ];
        assert!(LogDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn rejects_json() {
        let lines = vec![
            r#"{"name": "prezzy"}"#.into(),
        ];
        assert!(LogDetector.detect(&lines) < 0.1);
    }
}
