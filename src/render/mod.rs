mod csv;
mod diff;
pub mod json;
mod kv;
mod log;
mod markdown;
mod ndjson;
mod plain;
mod stacktrace;
mod xml;
mod yaml;

use std::io::{self, Write};

use anyhow::Result;

use crate::cli::Args;
use crate::detect::{self, DETECTION_BUFFER_SIZE, Format};
use crate::input::InputStream;
use crate::terminal::TerminalContext;
use crate::theme::Theme;

use self::csv::CsvRenderer;
use self::diff::DiffRenderer;
use self::json::JsonRenderer;
use self::kv::KeyValueRenderer;
use self::log::LogRenderer;
use self::markdown::MarkdownRenderer;
use self::ndjson::NdjsonRenderer;
use self::plain::PlainRenderer;
use self::stacktrace::StackTraceRenderer;
use self::xml::XmlRenderer;
use self::yaml::YamlRenderer;

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
    pub level_filter: Option<LevelFilter>,
    /// Use ASCII box-drawing instead of Unicode.
    pub ascii: bool,
}

/// Log level filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LevelFilter {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LevelFilter {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "trace" | "trc" | "verbose" => Some(Self::Trace),
            "debug" | "dbg" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" | "err" | "fatal" | "critical" | "crit" => Some(Self::Error),
            _ => None,
        }
    }

    #[must_use]
    pub fn passes(self, line_level: &str) -> bool {
        let Some(line) = Self::parse(line_level) else {
            return true;
        };
        line >= self
    }
}

/// Orchestrates detection and rendering.
pub struct RenderEngine<'a> {
    terminal: &'a TerminalContext,
    theme: &'a Theme,
    format_override: Option<crate::cli::FormatOverride>,
    level_filter: Option<LevelFilter>,
    ascii: bool,
}

impl<'a> RenderEngine<'a> {
    #[must_use]
    pub fn new(terminal: &'a TerminalContext, theme: &'a Theme, args: &Args) -> Self {
        let level_filter = args.level.as_deref().and_then(LevelFilter::parse);
        Self {
            terminal,
            theme,
            format_override: args.format,
            level_filter,
            ascii: args.ascii,
        }
    }

    /// Read input, detect format, and render to stdout.
    pub fn process(&mut self, input: &mut InputStream) -> Result<()> {
        let mut stdout = io::BufWriter::new(io::stdout().lock());

        let peeked = input.peek(DETECTION_BUFFER_SIZE)?;
        let format = detect::detect_format(peeked, self.format_override);

        let renderer = Self::renderer_for(format);
        let ctx = RenderContext {
            terminal: self.terminal,
            theme: self.theme,
            level_filter: self.level_filter,
            ascii: self.ascii,
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
            Format::Ndjson => Box::new(NdjsonRenderer),
            Format::Log => Box::new(LogRenderer),
            Format::Diff => Box::new(DiffRenderer),
            Format::StackTrace => Box::new(StackTraceRenderer),
            Format::Csv => Box::new(CsvRenderer::comma()),
            Format::Tsv => Box::new(CsvRenderer::tab()),
            Format::KeyValue => Box::new(KeyValueRenderer),
            Format::Yaml => Box::new(YamlRenderer),
            Format::Xml => Box::new(XmlRenderer),
            Format::Markdown => Box::new(MarkdownRenderer::new()),
            _ => Box::new(PlainRenderer),
        }
    }
}
