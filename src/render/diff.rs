use std::io::Write;

use anyhow::Result;
use crossterm::style::{Attribute, Stylize};

use super::{RenderContext, Renderer};

/// Renders unified diff output with color.
///
/// Line classification:
///   - `diff ...` / `index ...` / `--- ` / `+++ `  -> header (bold cyan)
///   - `@@ ... @@`                                   -> hunk header (cyan)
///   - `+...`                                        -> addition (green)
///   - `-...`                                        -> deletion (red)
///   - everything else                               -> context (dim)
pub struct DiffRenderer;

impl Renderer for DiffRenderer {
    fn render_line(&self, line: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        if !ctx.terminal.color_enabled {
            write!(writer, "{line}")?;
            return Ok(());
        }

        let colors = &ctx.theme.diff;
        let kind = classify_diff_line(line);

        match kind {
            DiffLine::Header => {
                write!(
                    writer,
                    "{}",
                    line.with(colors.header).attribute(Attribute::Bold)
                )?;
            }
            DiffLine::HunkHeader => {
                write!(writer, "{}", line.with(colors.header))?;
            }
            DiffLine::Add => {
                write!(writer, "{}", line.with(colors.add))?;
            }
            DiffLine::Remove => {
                write!(writer, "{}", line.with(colors.remove))?;
            }
            DiffLine::Context => {
                write!(writer, "{}", line.with(colors.context))?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffLine {
    Header,
    HunkHeader,
    Add,
    Remove,
    Context,
}

fn classify_diff_line(line: &str) -> DiffLine {
    if line.starts_with("diff ") || line.starts_with("index ") {
        return DiffLine::Header;
    }
    if line.starts_with("--- ") || line.starts_with("+++ ") {
        return DiffLine::Header;
    }
    if line.starts_with("@@ ") {
        return DiffLine::HunkHeader;
    }
    if line.starts_with('+') {
        return DiffLine::Add;
    }
    if line.starts_with('-') {
        return DiffLine::Remove;
    }
    DiffLine::Context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_diff_lines_correctly() {
        assert_eq!(classify_diff_line("diff --git a/f b/f"), DiffLine::Header);
        assert_eq!(classify_diff_line("--- a/file.rs"), DiffLine::Header);
        assert_eq!(classify_diff_line("+++ b/file.rs"), DiffLine::Header);
        assert_eq!(classify_diff_line("@@ -1,3 +1,4 @@"), DiffLine::HunkHeader);
        assert_eq!(classify_diff_line("+added line"), DiffLine::Add);
        assert_eq!(classify_diff_line("-removed line"), DiffLine::Remove);
        assert_eq!(classify_diff_line(" context line"), DiffLine::Context);
    }
}
