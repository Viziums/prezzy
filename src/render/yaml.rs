use std::io::Write;

use anyhow::Result;
use crossterm::style::{Color, Stylize};
use regex::Regex;
use std::sync::LazyLock;

use super::{RenderContext, Renderer};

/// Renders YAML with syntax highlighting.
///
/// Keys are cyan, string values green, numbers yellow,
/// booleans magenta, comments dim.
pub struct YamlRenderer;

static KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*)([\w][\w.\-/]*)\s*(:)\s*(.*)$").unwrap()
});

static COMMENT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*)(#.*)$").unwrap()
});

impl Renderer for YamlRenderer {
    fn render_line(&self, line: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        if !ctx.terminal.color_enabled {
            write!(writer, "{line}")?;
            return Ok(());
        }

        // Document markers.
        if line.trim() == "---" || line.trim() == "..." {
            write!(writer, "{}", line.with(Color::DarkGrey))?;
            return Ok(());
        }

        // Comments.
        if let Some(caps) = COMMENT_PATTERN.captures(line) {
            let indent = caps.get(1).map_or("", |m| m.as_str());
            let comment = caps.get(2).map_or("", |m| m.as_str());
            write!(writer, "{indent}{}", comment.with(Color::DarkGrey))?;
            return Ok(());
        }

        // Key: value lines.
        if let Some(caps) = KEY_PATTERN.captures(line) {
            let indent = caps.get(1).map_or("", |m| m.as_str());
            let key = caps.get(2).map_or("", |m| m.as_str());
            let colon = caps.get(3).map_or("", |m| m.as_str());
            let value = caps.get(4).map_or("", |m| m.as_str());

            write!(writer, "{indent}{}{} ", key.with(Color::Cyan), colon.with(Color::DarkGrey))?;
            write_yaml_value(value, writer)?;
            return Ok(());
        }

        // List items.
        let trimmed = line.trim_start();
        if let Some(content) = trimmed.strip_prefix("- ") {
            let indent_len = line.len() - trimmed.len();
            let indent = &line[..indent_len];
            write!(writer, "{indent}{} ", "-".with(Color::DarkGrey))?;
            write_yaml_value(content, writer)?;
            return Ok(());
        }

        write!(writer, "{line}")?;
        Ok(())
    }
}

fn write_yaml_value(value: &str, writer: &mut dyn Write) -> Result<()> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Ok(());
    }

    // Booleans.
    if matches!(trimmed, "true" | "false" | "yes" | "no" | "on" | "off" | "True" | "False" | "Yes" | "No") {
        write!(writer, "{}", trimmed.with(Color::Magenta))?;
        return Ok(());
    }

    // Null.
    if matches!(trimmed, "null" | "~" | "Null" | "NULL") {
        write!(writer, "{}", trimmed.with(Color::DarkGrey))?;
        return Ok(());
    }

    // Numbers.
    if trimmed.parse::<f64>().is_ok() {
        write!(writer, "{}", trimmed.with(Color::Yellow))?;
        return Ok(());
    }

    // Strings (with or without quotes).
    write!(writer, "{}", value.with(Color::Green))?;
    Ok(())
}
