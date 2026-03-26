mod json;
mod plain;

use std::io::{self, Write};

use anyhow::Result;

use crate::cli::Args;
use crate::detect::{self, Format, DETECTION_BUFFER_SIZE};
use crate::input::InputStream;
use crate::terminal::TerminalContext;
use crate::theme::Theme;

use self::json::JsonRenderer;
use self::plain::PlainRenderer;

/// Trait for format-specific renderers.
pub trait Renderer {
    /// Render a single line to the writer.
    fn render_line(&self, line: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()>;

    /// Whether this renderer needs all input before it can render.
    fn wants_full_input(&self) -> bool {
        false
    }

    /// Render the full accumulated input at once.
    fn render_all(&self, input: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        for line in input.lines() {
            self.render_line(line, writer, ctx)?;
        }
        Ok(())
    }
}

/// Shared context passed to every renderer.
pub struct RenderContext<'a> {
    pub terminal: &'a TerminalContext,
    pub theme: &'a Theme,
}

/// Orchestrates detection and rendering.
pub struct RenderEngine<'a> {
    terminal: &'a TerminalContext,
    theme: &'a Theme,
    format_override: Option<crate::cli::FormatOverride>,
}

impl<'a> RenderEngine<'a> {
    #[must_use] 
    pub const fn new(terminal: &'a TerminalContext, theme: &'a Theme, args: &Args) -> Self {
        Self {
            terminal,
            theme,
            format_override: args.format,
        }
    }

    /// Read input, detect format, and render to stdout.
    pub fn process(&mut self, input: &mut InputStream) -> Result<()> {
        let mut stdout = io::BufWriter::new(io::stdout().lock());

        // Buffer lines for detection.
        let peeked = input.peek(DETECTION_BUFFER_SIZE)?;
        let format = detect::detect_format(peeked, self.format_override);

        let renderer = Self::renderer_for(format);
        let ctx = RenderContext {
            terminal: self.terminal,
            theme: self.theme,
        };

        if renderer.wants_full_input() {
            let mut all = String::new();
            while let Some(line) = input.next_line()? {
                if !all.is_empty() {
                    all.push('\n');
                }
                all.push_str(&line);
            }
            renderer.render_all(&all, &mut stdout, &ctx)?;
            writeln!(stdout)?;
        } else {
            while let Some(line) = input.next_line()? {
                renderer.render_line(&line, &mut stdout, &ctx)?;
                writeln!(stdout)?;
            }
        }

        stdout.flush()?;
        Ok(())
    }

    fn renderer_for(format: Format) -> Box<dyn Renderer> {
        match format {
            Format::Json => Box::new(JsonRenderer),
            _ => Box::new(PlainRenderer),
        }
    }

}
