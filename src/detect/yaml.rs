use regex::Regex;
use std::sync::LazyLock;

use super::{Detector, Format};

/// Detects YAML content.
///
/// Signals: `key: value` patterns, `---` document start, consistent
/// indentation with spaces, list items `- item`.
pub struct YamlDetector;

/// Matches `key: value` (with no `=` sign, which would suggest KV format).
static KV_COLON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*[\w][\w.\-]*\s*:\s").unwrap()
});

/// Matches YAML list items `- item`.
static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*-\s+\S").unwrap()
});

impl Detector for YamlDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.is_empty() {
            return 0.0;
        }

        let mut doc_start = false;
        let mut colon_count = 0;
        let mut list_count = 0;
        let mut has_equals = false;
        let mut total_non_empty = 0;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            total_non_empty += 1;

            if trimmed == "---" || trimmed == "..." {
                doc_start = true;
                continue;
            }

            // If we see `=` signs, it's more likely KV or INI format.
            if trimmed.contains('=') && !trimmed.contains(": ") {
                has_equals = true;
            }

            if KV_COLON.is_match(line) {
                colon_count += 1;
            }

            if LIST_ITEM.is_match(line) {
                list_count += 1;
            }
        }

        if total_non_empty < 2 {
            return 0.0;
        }

        // If most lines have `=`, this is probably KV, not YAML.
        if has_equals {
            return 0.0;
        }

        let colon_ratio = f64::from(colon_count) / f64::from(total_non_empty);
        let list_ratio = f64::from(list_count) / f64::from(total_non_empty);

        // Document start marker is a strong signal.
        if doc_start && (colon_ratio > 0.3 || list_ratio > 0.3) {
            return 0.88;
        }

        // High ratio of `key: value` lines.
        if colon_ratio >= 0.6 {
            return 0.78;
        }

        // Mix of keys and list items.
        if colon_ratio + list_ratio >= 0.6 {
            return 0.7;
        }

        0.0
    }

    fn format(&self) -> Format {
        Format::Yaml
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_yaml_config() {
        let lines = vec![
            "server:".into(),
            "  host: localhost".into(),
            "  port: 8080".into(),
            "database:".into(),
            "  url: postgres://localhost/db".into(),
        ];
        assert!(YamlDetector.detect(&lines) > 0.7);
    }

    #[test]
    fn detects_yaml_with_doc_start() {
        let lines = vec![
            "---".into(),
            "name: prezzy".into(),
            "version: 0.1.0".into(),
        ];
        assert!(YamlDetector.detect(&lines) > 0.8);
    }

    #[test]
    fn rejects_key_value() {
        let lines = vec![
            "HOME=/home/user".into(),
            "PATH=/usr/bin".into(),
            "SHELL=/bin/bash".into(),
        ];
        assert!(YamlDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec!["hello world".into(), "just text".into()];
        assert!(YamlDetector.detect(&lines) < 0.1);
    }
}
