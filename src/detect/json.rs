use super::{Detector, Format};

/// Detects JSON objects and arrays.
pub struct JsonDetector;

impl Detector for JsonDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.is_empty() {
            return 0.0;
        }

        // Join all lines and try to parse as a single JSON value.
        let combined = lines.join("\n");
        let trimmed = combined.trim();

        if trimmed.is_empty() {
            return 0.0;
        }

        // Must start with { or [
        let first = trimmed.as_bytes().first().copied().unwrap_or(0);
        if first != b'{' && first != b'[' {
            return 0.0;
        }

        // Try to parse.
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(_) => 0.95,
            Err(_) => {
                // Might be a partial/truncated JSON — give moderate confidence
                // if it at least starts like JSON.
                if looks_like_json_start(trimmed) {
                    0.6
                } else {
                    0.0
                }
            }
        }
    }

    fn format(&self) -> Format {
        Format::Json
    }
}

/// Check if the string looks like the beginning of a JSON document.
fn looks_like_json_start(s: &str) -> bool {
    let s = s.trim();
    if s.starts_with('{') {
        // Looks like an object: expect `{"key`
        s.len() > 1 && s.contains('"')
    } else if s.starts_with('[') {
        // Array start
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_valid_json_object() {
        let lines = vec![r#"{"name": "prezzy", "version": "0.1.0"}"#.into()];
        assert!(JsonDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_valid_json_array() {
        let lines = vec!["[1, 2, 3]".into()];
        assert!(JsonDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_multiline_json() {
        let lines = vec![
            "{".into(),
            r#"  "name": "prezzy","#.into(),
            r#"  "version": "0.1.0""#.into(),
            "}".into(),
        ];
        assert!(JsonDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec!["hello world".into(), "this is not json".into()];
        assert!(JsonDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn rejects_empty_input() {
        let lines: Vec<String> = vec![];
        assert!(JsonDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn moderate_confidence_for_truncated_json() {
        let lines = vec![
            "{".into(),
            r#"  "name": "prezzy","#.into(),
            r#"  "items": ["#.into(),
        ];
        let score = JsonDetector.detect(&lines);
        assert!(score > 0.5);
        assert!(score < 0.9);
    }
}
