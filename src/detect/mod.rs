mod json;
mod plain;

pub use self::json::JsonDetector;
pub use self::plain::PlainDetector;

use crate::cli::FormatOverride;

/// The result of format detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Ndjson,
    Yaml,
    Xml,
    Csv,
    Tsv,
    Log,
    Diff,
    Markdown,
    KeyValue,
    Table,
    StackTrace,
    Plain,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Ndjson => write!(f, "ndjson"),
            Self::Yaml => write!(f, "yaml"),
            Self::Xml => write!(f, "xml"),
            Self::Csv => write!(f, "csv"),
            Self::Tsv => write!(f, "tsv"),
            Self::Log => write!(f, "log"),
            Self::Diff => write!(f, "diff"),
            Self::Markdown => write!(f, "markdown"),
            Self::KeyValue => write!(f, "kv"),
            Self::Table => write!(f, "table"),
            Self::StackTrace => write!(f, "stacktrace"),
            Self::Plain => write!(f, "plain"),
        }
    }
}

/// A detector scores how likely a set of lines matches a particular format.
pub trait Detector: Send + Sync {
    /// Inspect the buffered lines and return a confidence score in `0.0..=1.0`.
    fn detect(&self, lines: &[String]) -> f64;

    /// Which format this detector identifies.
    fn format(&self) -> Format;
}

/// Number of lines to buffer for format detection.
pub const DETECTION_BUFFER_SIZE: usize = 32;

/// Minimum confidence to accept a detection result.
const CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Run all registered detectors against the buffered lines and return the best match.
///
/// If a `FormatOverride` is provided, skip detection entirely.
#[must_use] 
pub fn detect_format(lines: &[String], force: Option<FormatOverride>) -> Format {
    if let Some(forced) = force {
        return override_to_format(forced);
    }

    let detectors: Vec<Box<dyn Detector>> = vec![
        Box::new(JsonDetector),
        // Future detectors:
        // Box::new(NdjsonDetector),
        // Box::new(DiffDetector),
        // Box::new(LogDetector),
    ];

    let mut best_format = Format::Plain;
    let mut best_score: f64 = 0.0;

    for detector in &detectors {
        let score = detector.detect(lines);
        if score > best_score && score >= CONFIDENCE_THRESHOLD {
            best_score = score;
            best_format = detector.format();
        }
    }

    best_format
}

const fn override_to_format(o: FormatOverride) -> Format {
    match o {
        FormatOverride::Json => Format::Json,
        FormatOverride::Ndjson => Format::Ndjson,
        FormatOverride::Yaml => Format::Yaml,
        FormatOverride::Xml => Format::Xml,
        FormatOverride::Csv => Format::Csv,
        FormatOverride::Tsv => Format::Tsv,
        FormatOverride::Log => Format::Log,
        FormatOverride::Diff => Format::Diff,
        FormatOverride::Markdown => Format::Markdown,
        FormatOverride::KeyValue => Format::KeyValue,
        FormatOverride::Table => Format::Table,
        FormatOverride::Plain => Format::Plain,
    }
}
