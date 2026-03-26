use std::io::Write;

use anyhow::Result;
use crossterm::style::Stylize;
use regex::Regex;
use std::sync::LazyLock;

use super::{RenderContext, Renderer};
use crate::theme::LogColors;

/// Renders plain-text log lines with level-based coloring.
pub struct LogRenderer;

/// Matches a timestamp at the start of a line (with optional brackets).
static TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^([\[\s]*",
        r"(?:",
            r"\d{4}[-/]\d{2}[-/]\d{2}",        // ISO date
            r"[T\s]\d{2}:\d{2}:\d{2}",         // time
            r"(?:[.,]\d+)?",                     // fractional seconds
            r"(?:Z|[+-]\d{2}:?\d{2})?",         // timezone
        r"|",
            r"[A-Z][a-z]{2}\s+\d{1,2}\s+",     // syslog month day
            r"\d{2}:\d{2}:\d{2}",               // time
        r"|",
            r"\d{2}:\d{2}:\d{2}",               // time only
            r"(?:[.,]\d+)?",                     // fractional seconds
        r")",
        r"\]?\s*)",
    )).unwrap()
});

/// Matches log level keyword.
static LEVEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(ERROR|ERR|FATAL|CRITICAL|CRIT|WARN(?:ING)?|INFO|DEBUG|DBG|TRACE|TRC|VERBOSE)\b").unwrap()
});

impl Renderer for LogRenderer {
    fn render_line(&self, line: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        // Apply level filter if set.
        if let Some(filter) = ctx.level_filter {
            if let Some(m) = LEVEL_RE.find(line) {
                if !filter.passes(m.as_str()) {
                    return Ok(());
                }
            }
        }

        if !ctx.terminal.color_enabled {
            write!(writer, "{line}")?;
            return Ok(());
        }

        let colors = &ctx.theme.log;
        let mut rest = line;

        // 1. Dim the timestamp prefix.
        if let Some(m) = TIMESTAMP_RE.find(rest) {
            write!(writer, "{}", m.as_str().with(colors.timestamp))?;
            rest = &rest[m.end()..];
        }

        // 2. Color the level keyword.
        if let Some(m) = LEVEL_RE.find(rest) {
            write!(writer, "{}", &rest[..m.start()])?;
            write_level(writer, m.as_str(), colors)?;
            rest = &rest[m.end()..];
        }

        // 3. Rest of the line unmodified.
        write!(writer, "{rest}")?;

        Ok(())
    }
}

fn write_level(writer: &mut dyn Write, level: &str, colors: &LogColors) -> Result<()> {
    let upper = level.to_uppercase();
    let color = match upper.as_str() {
        "ERROR" | "ERR" | "FATAL" | "CRITICAL" | "CRIT" => colors.error,
        "WARN" | "WARNING" => colors.warn,
        "DEBUG" | "DBG" => colors.debug,
        "TRACE" | "TRC" | "VERBOSE" => colors.trace,
        _ => colors.info, // INFO and unrecognized levels
    };
    write!(writer, "{}", level.with(color))?;
    Ok(())
}
