use super::{Detector, Format};

/// Fallback detector that always matches with low confidence.
pub struct PlainDetector;

impl Detector for PlainDetector {
    fn detect(&self, _lines: &[String]) -> f64 {
        // Plain text is always the fallback. The orchestrator picks it
        // when nothing else exceeds the confidence threshold.
        0.1
    }

    fn format(&self) -> Format {
        Format::Plain
    }
}
