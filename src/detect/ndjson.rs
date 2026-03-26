use super::{Detector, Format};

/// Detects newline-delimited JSON (NDJSON / JSON Lines).
///
/// Each line is an independent JSON object. Common in structured logging
/// (Bunyan, pino, Winston JSON, Docker JSON logs).
pub struct NdjsonDetector;

impl Detector for NdjsonDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.len() < 2 {
            return 0.0;
        }

        let mut json_lines = 0;
        let mut total_non_empty = 0;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            total_non_empty += 1;

            // Must start with '{' -- NDJSON is objects, not arrays.
            if !trimmed.starts_with('{') {
                continue;
            }

            if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                json_lines += 1;
            }
        }

        if total_non_empty < 2 {
            return 0.0;
        }

        let ratio = f64::from(json_lines) / f64::from(total_non_empty);

        // Need at least 80% of lines to be valid JSON objects.
        if ratio >= 0.8 {
            // Higher confidence than single JSON (0.95) so NDJSON wins
            // when both could match.
            0.97
        } else if ratio >= 0.5 {
            0.6
        } else {
            0.0
        }
    }

    fn format(&self) -> Format {
        Format::Ndjson
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ndjson() {
        let lines = vec![
            r#"{"level":"info","msg":"started"}"#.into(),
            r#"{"level":"error","msg":"failed"}"#.into(),
            r#"{"level":"info","msg":"done"}"#.into(),
        ];
        assert!(NdjsonDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn rejects_single_json_object() {
        let lines = vec![r#"{"name":"prezzy"}"#.into()];
        assert!(NdjsonDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec!["hello".into(), "world".into()];
        assert!(NdjsonDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn handles_mixed_with_empty_lines() {
        let lines = vec![r#"{"a":1}"#.into(), String::new(), r#"{"b":2}"#.into()];
        assert!(NdjsonDetector.detect(&lines) > 0.9);
    }
}
