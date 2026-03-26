use std::io::Write;

use anyhow::Result;
use crossterm::style::{Attribute, Color, Stylize};

use super::{RenderContext, Renderer};

/// Renders Markdown with inline syntax highlighting.
///
/// Headings are bold cyan, code fences dim, blockquotes gray,
/// list markers colored, bold/italic preserved.
pub struct MarkdownRenderer {
    in_code_block: std::cell::Cell<bool>,
}

impl MarkdownRenderer {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            in_code_block: std::cell::Cell::new(false),
        }
    }
}

impl Renderer for MarkdownRenderer {
    fn render_line(&self, line: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        if !ctx.terminal.color_enabled {
            write!(writer, "{line}")?;
            return Ok(());
        }

        let trimmed = line.trim();

        // Code fence toggle.
        if trimmed.starts_with("```") {
            self.in_code_block.set(!self.in_code_block.get());
            write!(writer, "{}", line.with(Color::DarkGrey))?;
            return Ok(());
        }

        // Inside code block: dim monospace.
        if self.in_code_block.get() {
            write!(writer, "{}", line.with(Color::Green))?;
            return Ok(());
        }

        // Headings.
        if trimmed.starts_with('#') {
            write!(writer, "{}", line.with(Color::Cyan).attribute(Attribute::Bold))?;
            return Ok(());
        }

        // Horizontal rules.
        if matches!(trimmed, "---" | "***" | "___") {
            write!(writer, "{}", line.with(Color::DarkGrey))?;
            return Ok(());
        }

        // Blockquotes.
        if let Some(content) = trimmed.strip_prefix("> ") {
            let indent = &line[..line.len() - trimmed.len()];
            write!(writer, "{}{} {content}", indent, ">".with(Color::DarkGrey))?;
            return Ok(());
        }

        // List items.
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let indent = &line[..line.len() - trimmed.len()];
            let marker = &trimmed[..1];
            let rest = &trimmed[2..];
            write!(writer, "{}{} {rest}", indent, marker.with(Color::Yellow))?;
            return Ok(());
        }

        // Numbered list.
        if let Some(dot_pos) = trimmed.find(". ") {
            if dot_pos <= 3 && trimmed[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
                let indent = &line[..line.len() - trimmed.len()];
                let number = &trimmed[..=dot_pos];
                let rest = &trimmed[dot_pos + 2..];
                write!(writer, "{}{} {rest}", indent, number.with(Color::Yellow))?;
                return Ok(());
            }
        }

        // Plain text.
        write!(writer, "{line}")?;
        Ok(())
    }
}
