use std::io::Write;

use anyhow::Result;
use crossterm::style::{self, Stylize};

use super::{RenderContext, Renderer};

/// Renders JSON with syntax highlighting and pretty indentation.
pub struct JsonRenderer;

impl Renderer for JsonRenderer {
    fn wants_full_input(&self) -> bool {
        true
    }

    fn render_line(&self, line: &str, writer: &mut dyn Write, _ctx: &RenderContext) -> Result<()> {
        write!(writer, "{line}")?;
        Ok(())
    }

    fn render_all(&self, input: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        let trimmed = input.trim();

        // Try to parse and re-serialize with indentation.
        let value: serde_json::Value = if let Ok(v) = serde_json::from_str(trimmed) { v } else {
            // Not valid JSON -- fall through to plain output.
            write!(writer, "{input}")?;
            return Ok(());
        };

        let pretty = serde_json::to_string_pretty(&value)?;

        if ctx.terminal.color_enabled {
            highlight_json(&pretty, writer, ctx)?;
        } else {
            // No color: still pretty-print (indentation), just no ANSI codes.
            write!(writer, "{pretty}")?;
        }

        Ok(())
    }
}

/// Walk the pretty-printed JSON string and apply colors character by character.
fn highlight_json(json: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
    let theme = &ctx.theme;
    let mut in_string = false;
    let mut is_key = false;
    let mut escape_next = false;
    let mut after_colon = false;

    for ch in json.chars() {
        if escape_next {
            write_colored(writer, ch, if is_key { theme.json_key } else { theme.json_string })?;
            escape_next = false;
            continue;
        }

        if ch == '\\' && in_string {
            write_colored(writer, ch, if is_key { theme.json_key } else { theme.json_string })?;
            escape_next = true;
            continue;
        }

        if ch == '"' {
            if in_string {
                write_colored(writer, ch, if is_key { theme.json_key } else { theme.json_string })?;
                in_string = false;
                is_key = false;
            } else {
                is_key = !after_colon;
                write_colored(writer, ch, if is_key { theme.json_key } else { theme.json_string })?;
                in_string = true;
            }
            continue;
        }

        if in_string {
            write_colored(writer, ch, if is_key { theme.json_key } else { theme.json_string })?;
            continue;
        }

        match ch {
            ':' => {
                write_colored(writer, ch, theme.json_bracket)?;
                after_colon = true;
            }
            ',' | '\n' => {
                after_colon = false;
                write!(writer, "{ch}")?;
            }
            '{' | '}' | '[' | ']' => {
                after_colon = false;
                write_colored(writer, ch, theme.json_bracket)?;
            }
            _ if ch.is_ascii_digit() || ch == '-' || ch == '.' => {
                write_colored(writer, ch, theme.json_number)?;
            }
            't' | 'f' | 'r' | 'u' | 'e' | 'a' | 'l' | 's' => {
                write_colored(writer, ch, theme.json_bool)?;
            }
            'n' => {
                write_colored(writer, ch, theme.json_null)?;
            }
            _ => {
                write!(writer, "{ch}")?;
            }
        }
    }

    Ok(())
}

fn write_colored(writer: &mut dyn Write, ch: char, color: style::Color) -> Result<()> {
    write!(writer, "{}", format!("{ch}").with(color))?;
    Ok(())
}
