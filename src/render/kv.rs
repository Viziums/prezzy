use std::io::Write;

use anyhow::Result;
use crossterm::style::{Color, Stylize};

use super::{RenderContext, Renderer};

/// Renders KEY=VALUE lines with aligned columns and colored keys.
///
/// Accumulates all input to calculate max key width, then renders
/// with consistent alignment.
pub struct KeyValueRenderer;

impl Renderer for KeyValueRenderer {
    fn wants_full_input(&self) -> bool {
        true
    }

    fn render_line(&self, line: &str, writer: &mut dyn Write, _ctx: &RenderContext) -> Result<()> {
        write!(writer, "{line}")?;
        Ok(())
    }

    fn render_all(&self, input: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        let mut entries: Vec<(&str, &str)> = Vec::new();
        let mut max_key_len: usize = 0;

        for line in input.lines() {
            if let Some((key, value)) = split_kv(line) {
                max_key_len = max_key_len.max(key.len());
                entries.push((key, value));
            } else {
                entries.push(("", line));
            }
        }

        // Cap alignment to something reasonable.
        let pad = max_key_len.min(40);

        for (i, (key, value)) in entries.iter().enumerate() {
            if i > 0 {
                writeln!(writer)?;
            }

            if key.is_empty() {
                // Comment or blank line -- pass through.
                if ctx.terminal.color_enabled {
                    write!(writer, "{}", value.with(Color::DarkGrey))?;
                } else {
                    write!(writer, "{value}")?;
                }
                continue;
            }

            if ctx.terminal.color_enabled {
                write!(
                    writer,
                    "{:<pad$} {} {}",
                    key.with(Color::Cyan),
                    "=".with(Color::DarkGrey),
                    value,
                )?;
            } else {
                write!(writer, "{key:<pad$} = {value}")?;
            }
        }

        Ok(())
    }
}

/// Split a line into key and value at the first `=`.
fn split_kv(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();

    // Skip comments and blank lines.
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let eq_pos = trimmed.find('=')?;
    let key = trimmed[..eq_pos].trim_end();
    let value = trimmed[eq_pos + 1..].trim_start();

    // Key must look like an identifier.
    if key.is_empty() || !key.as_bytes()[0].is_ascii_alphabetic() && key.as_bytes()[0] != b'_' {
        return None;
    }

    Some((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_kv() {
        assert_eq!(split_kv("FOO=bar"), Some(("FOO", "bar")));
        assert_eq!(split_kv("KEY = value"), Some(("KEY", "value")));
        assert_eq!(split_kv("EMPTY="), Some(("EMPTY", "")));
    }

    #[test]
    fn skips_non_kv() {
        assert_eq!(split_kv("# comment"), None);
        assert_eq!(split_kv(""), None);
        assert_eq!(split_kv("no equals here"), None);
    }
}
