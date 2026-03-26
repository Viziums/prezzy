use super::{Detector, Format};

/// Detects Markdown content.
///
/// Signals: `#` headings, `- ` / `* ` / `1. ` lists, triple backtick
/// code fences, `> ` blockquotes, `---` / `***` horizontal rules.
pub struct MarkdownDetector;

impl Detector for MarkdownDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.is_empty() {
            return 0.0;
        }

        let mut heading_count = 0;
        let mut list_count = 0;
        let mut code_fence_count = 0;
        let mut blockquote_count = 0;
        let mut total_non_empty = 0;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            total_non_empty += 1;

            if trimmed.starts_with('#') && trimmed.contains("# ") {
                heading_count += 1;
            } else if trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || (trimmed.len() > 2
                    && trimmed.as_bytes()[0].is_ascii_digit()
                    && trimmed.contains(". "))
            {
                list_count += 1;
            } else if trimmed.starts_with("```") {
                code_fence_count += 1;
            } else if trimmed.starts_with("> ") {
                blockquote_count += 1;
            }
        }

        if total_non_empty < 2 {
            return 0.0;
        }

        let signal_count = heading_count + code_fence_count + blockquote_count;

        // Headings + code fences are strong markdown signals.
        if heading_count >= 1 && (code_fence_count >= 1 || list_count >= 2) {
            return 0.82;
        }

        if heading_count >= 2 {
            return 0.78;
        }

        // Single heading + list items.
        if heading_count >= 1 && list_count >= 1 {
            return 0.7;
        }

        // Code fences alone.
        if code_fence_count >= 2 {
            return 0.65;
        }

        // Mix of signals.
        if signal_count >= 2 {
            return 0.6;
        }

        0.0
    }

    fn format(&self) -> Format {
        Format::Markdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_markdown() {
        let lines = vec![
            "# Title".into(),
            String::new(),
            "Some text here.".into(),
            String::new(),
            "## Section".into(),
            String::new(),
            "- item one".into(),
            "- item two".into(),
        ];
        assert!(MarkdownDetector.detect(&lines) > 0.7);
    }

    #[test]
    fn detects_markdown_with_code() {
        let lines = vec![
            "# README".into(),
            String::new(),
            "```rust".into(),
            "fn main() {}".into(),
            "```".into(),
        ];
        assert!(MarkdownDetector.detect(&lines) > 0.8);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec!["hello world".into(), "just text".into()];
        assert!(MarkdownDetector.detect(&lines) < 0.1);
    }
}
