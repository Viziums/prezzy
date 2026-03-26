use super::{Detector, Format};

/// Detects CSV (comma-separated) data.
///
/// Strategy: check if lines have a consistent number of a candidate
/// delimiter (comma, semicolon, pipe). Tab-separated is handled by
/// `TsvDetector` separately.
pub struct CsvDetector;

/// Detects TSV (tab-separated) data.
pub struct TsvDetector;

impl Detector for CsvDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        detect_delimited(lines, &[',', ';', '|'])
    }

    fn format(&self) -> Format {
        Format::Csv
    }
}

impl Detector for TsvDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        detect_delimited(lines, &['\t'])
    }

    fn format(&self) -> Format {
        Format::Tsv
    }
}

/// Core detection logic: find a delimiter that produces consistent
/// column counts across lines.
fn detect_delimited(lines: &[String], candidates: &[char]) -> f64 {
    if lines.len() < 2 {
        return 0.0;
    }

    let mut best_score: f64 = 0.0;

    for &delim in candidates {
        let counts: Vec<usize> = lines
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| count_delimiter(l, delim))
            .collect();

        if counts.len() < 2 {
            continue;
        }

        // Need at least 1 delimiter per line (i.e., at least 2 columns).
        let first = counts[0];
        if first == 0 {
            continue;
        }

        // Count how many lines have the same delimiter count as the first line.
        let consistent = counts.iter().filter(|&&c| c == first).count();

        #[allow(clippy::cast_precision_loss)]
        let ratio = consistent as f64 / counts.len() as f64;

        // Strong signal: nearly all lines have the same column count.
        let score = if ratio >= 0.9 && first >= 2 {
            0.88
        } else if ratio >= 0.9 && first >= 1 {
            0.78
        } else if ratio >= 0.7 && first >= 1 {
            0.6
        } else {
            0.0
        };

        if score > best_score {
            best_score = score;
        }
    }

    best_score
}

/// Count occurrences of `delim` in `line`, respecting quoted fields.
fn count_delimiter(line: &str, delim: char) -> usize {
    let mut count = 0;
    let mut in_quotes = false;
    let mut prev_was_escape = false;

    for ch in line.chars() {
        if prev_was_escape {
            prev_was_escape = false;
            continue;
        }
        if ch == '\\' {
            prev_was_escape = true;
            continue;
        }
        if ch == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if ch == delim && !in_quotes {
            count += 1;
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_csv() {
        let lines = vec![
            "name,age,city".into(),
            "Alice,30,NYC".into(),
            "Bob,25,London".into(),
        ];
        assert!(CsvDetector.detect(&lines) > 0.8);
    }

    #[test]
    fn detects_tsv() {
        let lines = vec![
            "name\tage\tcity".into(),
            "Alice\t30\tNYC".into(),
            "Bob\t25\tLondon".into(),
        ];
        assert!(TsvDetector.detect(&lines) > 0.8);
    }

    #[test]
    fn detects_semicolon_csv() {
        let lines = vec![
            "name;age;city".into(),
            "Alice;30;NYC".into(),
            "Bob;25;London".into(),
        ];
        assert!(CsvDetector.detect(&lines) > 0.8);
    }

    #[test]
    fn handles_quoted_fields() {
        let lines = vec![
            r"name,description,value".into(),
            r#"Alice,"hello, world",42"#.into(),
            r#"Bob,"foo, bar",99"#.into(),
        ];
        // All lines should have 2 commas (the comma inside quotes doesn't count).
        assert!(CsvDetector.detect(&lines) > 0.7);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec!["hello world".into(), "no delimiters here".into()];
        assert!(CsvDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn rejects_inconsistent_columns() {
        let lines = vec!["a,b,c".into(), "d,e".into(), "f".into()];
        assert!(CsvDetector.detect(&lines) < 0.5);
    }
}
