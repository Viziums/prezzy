use std::io::Write;

use anyhow::Result;
use crossterm::style::{Color, Stylize};

use super::{RenderContext, Renderer};

/// Renders XML/HTML with syntax highlighting.
///
/// Tags are cyan, attributes yellow, attribute values green,
/// text content plain, comments dim.
pub struct XmlRenderer;

impl Renderer for XmlRenderer {
    fn render_line(&self, line: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        if !ctx.terminal.color_enabled {
            write!(writer, "{line}")?;
            return Ok(());
        }

        let mut i = 0;
        let bytes = line.as_bytes();
        let len = bytes.len();

        while i < len {
            if bytes[i] == b'<' {
                // Check for comment.
                if line[i..].starts_with("<!--") {
                    if let Some(end) = line[i..].find("-->") {
                        let comment = &line[i..i + end + 3];
                        write!(writer, "{}", comment.with(Color::DarkGrey))?;
                        i += end + 3;
                        continue;
                    }
                }

                // Find the end of the tag.
                let tag_start = i;
                i += 1;
                let mut in_attr_value = false;
                while i < len {
                    if bytes[i] == b'"' {
                        in_attr_value = !in_attr_value;
                    }
                    if bytes[i] == b'>' && !in_attr_value {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let tag = &line[tag_start..i];
                write_highlighted_tag(tag, writer)?;
            } else {
                // Text content until next `<`.
                let start = i;
                while i < len && bytes[i] != b'<' {
                    i += 1;
                }
                write!(writer, "{}", &line[start..i])?;
            }
        }

        Ok(())
    }
}

/// Highlight a single XML tag like `<tag attr="val">` or `</tag>`.
fn write_highlighted_tag(tag: &str, writer: &mut dyn Write) -> Result<()> {
    let inner = tag.trim_start_matches('<').trim_end_matches('>');

    write!(writer, "{}", "<".with(Color::Cyan))?;

    let chars = inner.chars();
    let mut in_tag_name = true;
    let mut in_attr_name = false;
    let mut in_attr_value = false;
    let mut buf = String::new();

    for ch in chars {
        if in_attr_value {
            buf.push(ch);
            if ch == '"' {
                write!(writer, "{}", buf.as_str().with(Color::Green))?;
                buf.clear();
                in_attr_value = false;
                in_attr_name = false;
            }
            continue;
        }

        if ch == '"' {
            // Flush attr name.
            if !buf.is_empty() {
                write!(writer, "{}", buf.as_str().with(Color::Yellow))?;
                buf.clear();
            }
            buf.push(ch);
            in_attr_value = true;
            continue;
        }

        if in_tag_name {
            if ch.is_whitespace() {
                write!(writer, "{}", buf.as_str().with(Color::Cyan))?;
                buf.clear();
                write!(writer, "{ch}")?;
                in_tag_name = false;
                in_attr_name = true;
                continue;
            }
            buf.push(ch);
            continue;
        }

        if ch == '=' {
            // Flush attribute name.
            write!(writer, "{}", buf.as_str().with(Color::Yellow))?;
            buf.clear();
            write!(writer, "{}", "=".with(Color::DarkGrey))?;
            in_attr_name = false;
            continue;
        }

        if ch.is_whitespace() && !in_attr_name {
            write!(writer, "{ch}")?;
            in_attr_name = true;
            continue;
        }

        buf.push(ch);
    }

    // Flush remaining buffer.
    if !buf.is_empty() {
        if in_tag_name {
            write!(writer, "{}", buf.as_str().with(Color::Cyan))?;
        } else {
            write!(writer, "{}", buf.as_str().with(Color::Yellow))?;
        }
    }

    write!(writer, "{}", ">".with(Color::Cyan))?;
    Ok(())
}
