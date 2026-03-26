use super::{Detector, Format};

/// Detects XML/HTML content.
///
/// Looks for `<tag>` patterns, `<?xml` declarations, and `<!DOCTYPE`.
pub struct XmlDetector;

impl Detector for XmlDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.is_empty() {
            return 0.0;
        }

        let mut tag_count = 0;
        let mut has_declaration = false;
        let mut total_non_empty = 0;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            total_non_empty += 1;

            if trimmed.starts_with("<?xml") || trimmed.starts_with("<!DOCTYPE") {
                has_declaration = true;
                continue;
            }

            if trimmed.starts_with('<') && trimmed.contains('>') {
                tag_count += 1;
            }
        }

        if total_non_empty == 0 {
            return 0.0;
        }

        if has_declaration {
            return 0.92;
        }

        let ratio = f64::from(tag_count) / f64::from(total_non_empty);

        if ratio >= 0.6 && tag_count >= 3 {
            0.80
        } else if ratio >= 0.4 && tag_count >= 2 {
            0.6
        } else {
            0.0
        }
    }

    fn format(&self) -> Format {
        Format::Xml
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_xml_with_declaration() {
        let lines = vec![
            r#"<?xml version="1.0" encoding="UTF-8"?>"#.into(),
            "<root>".into(),
            "  <item>hello</item>".into(),
            "</root>".into(),
        ];
        assert!(XmlDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_html() {
        let lines = vec![
            "<!DOCTYPE html>".into(),
            "<html>".into(),
            "<head><title>Test</title></head>".into(),
            "<body><p>Hello</p></body>".into(),
            "</html>".into(),
        ];
        assert!(XmlDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_xml_without_declaration() {
        let lines = vec![
            "<config>".into(),
            "  <server host=\"localhost\" port=\"8080\"/>".into(),
            "  <database url=\"postgres://localhost/db\"/>".into(),
            "</config>".into(),
        ];
        assert!(XmlDetector.detect(&lines) > 0.7);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec!["hello world".into(), "not xml".into()];
        assert!(XmlDetector.detect(&lines) < 0.1);
    }
}
