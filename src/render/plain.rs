use std::io::Write;

use anyhow::Result;

use super::{RenderContext, Renderer};

/// Passthrough renderer with minimal enhancements.
///
/// Currently outputs text unchanged. Future enhancements:
/// - URL highlighting
/// - Smart line wrapping at terminal width
pub struct PlainRenderer;

impl Renderer for PlainRenderer {
    fn render_line(&self, line: &str, writer: &mut dyn Write, _ctx: &RenderContext) -> Result<()> {
        write!(writer, "{line}")?;
        Ok(())
    }
}
