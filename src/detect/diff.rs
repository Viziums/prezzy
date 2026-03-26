use super::{Detector, Format};

/// Detects unified diff / patch output.
///
/// Unified diffs have very distinctive markers:
///   - `diff --git a/file b/file` (git diff header)
///   - `--- a/file` / `+++ b/file` (file headers)
///   - `@@ -N,M +N,M @@` (hunk headers)
///   - Lines starting with `+` or `-` (additions/deletions)
pub struct DiffDetector;

impl Detector for DiffDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.is_empty() {
            return 0.0;
        }

        let mut has_diff_header = false;
        let mut has_file_header = false;
        let mut has_hunk_header = false;
        let mut add_remove_count: i32 = 0;
        let mut total_non_empty: i32 = 0;

        for line in lines {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            total_non_empty += 1;

            if trimmed.starts_with("diff ") {
                has_diff_header = true;
            } else if trimmed.starts_with("--- ") || trimmed.starts_with("+++ ") {
                has_file_header = true;
            } else if trimmed.starts_with("@@ ") {
                has_hunk_header = true;
            } else if trimmed.starts_with('+') || trimmed.starts_with('-') {
                add_remove_count += 1;
            }
        }

        if has_hunk_header {
            return 0.95;
        }

        if has_diff_header && has_file_header {
            return 0.92;
        }

        if has_file_header && add_remove_count > 0 {
            return 0.85;
        }

        if total_non_empty > 0 {
            let ratio = f64::from(add_remove_count) / f64::from(total_non_empty);
            if ratio > 0.5 && add_remove_count >= 3 {
                return 0.55;
            }
        }

        0.0
    }

    fn format(&self) -> Format {
        Format::Diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_git_diff() {
        let lines = vec![
            "diff --git a/src/main.rs b/src/main.rs".into(),
            "index abc1234..def5678 100644".into(),
            "--- a/src/main.rs".into(),
            "+++ b/src/main.rs".into(),
            "@@ -1,3 +1,4 @@".into(),
            " use std::io;".into(),
            "+use std::fs;".into(),
        ];
        assert!(DiffDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_plain_unified_diff() {
        let lines = vec![
            "--- old_file.txt\t2024-01-15".into(),
            "+++ new_file.txt\t2024-01-16".into(),
            "@@ -1,2 +1,3 @@".into(),
            " unchanged".into(),
            "-removed line".into(),
            "+added line".into(),
        ];
        assert!(DiffDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec!["hello world".into(), "not a diff".into()];
        assert!(DiffDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn rejects_markdown_lists() {
        let lines = vec![
            "- item one".into(),
            "- item two".into(),
            "- item three".into(),
        ];
        assert!(DiffDetector.detect(&lines) < 0.6);
    }
}
