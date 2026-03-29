//! Output beautifier for shell mode.
//!
//! Buffers the first N lines of a command's output, runs format detection,
//! and then either:
//!   - renders through the appropriate format renderer (JSON, diff, …), or
//!   - falls back to raw passthrough when no format is detected.
//!
//! The beautifier is a state machine driven by the output loop:
//!
//! ```text
//!  Idle ──[command start]──► Buffering
//!    ▲                          │
//!    │                    ┌─────┴──────┐
//!    │                  detect        detect
//!    │                 found!       not found
//!    │                    │            │
//!    │                    ▼            ▼
//!    │              Rendering     Passthrough
//!    │                    │            │
//!    └──[command end]─────┴────────────┘
//! ```

use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::detect::{self, DETECTION_BUFFER_SIZE, Format};
use crate::render::{self, LevelFilter, RenderContext, Renderer};
use crate::terminal::{ColorDepth, TerminalContext};
use crate::theme::Theme;

/// Maximum raw bytes to buffer before forcing a passthrough fallback.
/// Prevents OOM on commands that produce huge output (e.g. `yes`, `cat /dev/urandom`).
const MAX_RAW_BUFFER: usize = 1024 * 1024; // 1 MiB

/// Maximum clean lines to keep for full-input renderers.
/// Beyond this we flush as-is to avoid unbounded memory growth.
const MAX_CLEAN_LINES: usize = 50_000;

/// Time to wait for enough lines before forcing detection with whatever we have.
/// Prevents the user from seeing nothing when a slow command outputs fewer than
/// `DETECTION_BUFFER_SIZE` lines then blocks.
const DETECTION_TIMEOUT: Duration = Duration::from_millis(50);

/// State machine for the beautifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Not currently processing a command.
    Idle,
    /// Collecting the first N lines for format detection.
    Buffering,
    /// Format detected — rendering line by line.
    Rendering,
    /// Format detected but renderer needs all input at once (JSON, CSV).
    /// We keep buffering clean lines until the command ends.
    RenderingFull,
    /// No format detected — passing raw bytes through unchanged.
    Passthrough,
}

/// Per-command output beautifier.
///
/// Create once and reuse across commands — call [`start`] at the beginning
/// of each command and [`finish`] at the end.
pub struct OutputBeautifier<'a> {
    theme: &'a Theme,
    terminal: TerminalContext,
    level_filter: Option<LevelFilter>,
    ascii: bool,

    state: State,
    /// When buffering started — used for the detection timeout.
    buffering_start: Option<Instant>,
    /// Raw PTY bytes buffered while in `Buffering` state (for passthrough fallback).
    raw_buffer: Vec<u8>,
    /// Clean text lines accumulated for detection and rendering.
    clean_lines: Vec<String>,
    /// The renderer selected after detection.
    renderer: Option<Box<dyn Renderer>>,
}

impl<'a> OutputBeautifier<'a> {
    pub fn new(theme: &'a Theme, level_filter: Option<LevelFilter>, ascii: bool) -> Self {
        let (width, _) = crossterm::terminal::size().unwrap_or((80, 24));
        let terminal = TerminalContext {
            color_enabled: true,
            color_depth: ColorDepth::detect(),
            width,
            is_tty: true,
        };

        Self {
            theme,
            terminal,
            level_filter,
            ascii,
            state: State::Idle,
            buffering_start: None,
            raw_buffer: Vec::with_capacity(8192),
            clean_lines: Vec::with_capacity(DETECTION_BUFFER_SIZE),
            renderer: None,
        }
    }

    // -- lifecycle ------------------------------------------------------------

    /// Begin buffering for a new command.
    pub fn start(&mut self) {
        self.state = State::Buffering;
        self.buffering_start = Some(Instant::now());
        self.raw_buffer.clear();
        self.clean_lines.clear();
        self.renderer = None;
    }

    /// Feed raw PTY bytes (for passthrough fallback).
    ///
    /// If the raw buffer exceeds [`MAX_RAW_BUFFER`], returns `true` to signal
    /// the caller should force a flush to passthrough.
    pub fn feed_raw(&mut self, raw: &[u8]) -> bool {
        if matches!(self.state, State::Buffering | State::RenderingFull) {
            self.raw_buffer.extend_from_slice(raw);
            return self.raw_buffer.len() > MAX_RAW_BUFFER;
        }
        false
    }

    /// Feed clean text lines extracted by the VTE parser.
    pub fn feed_lines(&mut self, lines: Vec<String>) {
        if matches!(
            self.state,
            State::Buffering | State::RenderingFull
        ) {
            self.clean_lines.extend(lines);
        }
    }

    /// Whether the buffer has enough lines (or enough time has passed) to
    /// attempt format detection.
    ///
    /// The timeout ensures the user sees output even when a slow command
    /// produces fewer than `DETECTION_BUFFER_SIZE` lines then blocks.
    /// Note: the timeout only triggers when new PTY output arrives (the loop
    /// blocks on `read`), so it helps with slow-but-not-stopped output.
    pub fn should_detect(&self) -> bool {
        self.state == State::Buffering
            && (self.clean_lines.len() >= DETECTION_BUFFER_SIZE
                || self
                    .buffering_start
                    .is_some_and(|t| t.elapsed() >= DETECTION_TIMEOUT))
    }

    /// Whether buffers have exceeded safety limits and must be flushed.
    pub fn over_limit(&self) -> bool {
        self.raw_buffer.len() > MAX_RAW_BUFFER || self.clean_lines.len() > MAX_CLEAN_LINES
    }

    /// Force-flush to passthrough because buffers exceeded limits.
    pub fn force_passthrough(&mut self, w: &mut impl Write) -> Result<()> {
        if !self.raw_buffer.is_empty() {
            w.write_all(&self.raw_buffer)?;
            w.flush()?;
        }
        self.raw_buffer.clear();
        self.clean_lines.clear();
        self.renderer = None;
        self.state = State::Passthrough;
        Ok(())
    }

    /// Run format detection on the buffered lines and transition state.
    ///
    /// If a format is found, renders the buffered lines immediately (for
    /// line-by-line renderers) or keeps buffering (for full-input renderers).
    /// If no format is found, flushes raw bytes and switches to passthrough.
    pub fn detect_and_render(&mut self, w: &mut impl Write) -> Result<()> {
        let format = detect::detect_format(&self.clean_lines, None);

        if matches!(format, Format::Plain) {
            // No interesting format — dump raw and switch to passthrough.
            w.write_all(&self.raw_buffer)?;
            w.flush()?;
            self.raw_buffer.clear();
            self.state = State::Passthrough;
            return Ok(());
        }

        let renderer = render::renderer_for(format);

        if renderer.wants_full_input() {
            // Keep collecting until the command ends.
            self.renderer = Some(renderer);
            self.state = State::RenderingFull;
            return Ok(());
        }

        // Render the buffered lines immediately.
        let ctx = self.render_context();
        for line in &self.clean_lines {
            renderer.render_line(line, w, &ctx)?;
            writeln!(w)?;
        }
        w.flush()?;

        self.renderer = Some(renderer);
        self.raw_buffer.clear();
        self.clean_lines.clear();
        self.state = State::Rendering;

        Ok(())
    }

    /// Render newly arrived clean lines (called when in `Rendering` state).
    pub fn render_lines(&self, lines: &[String], w: &mut impl Write) -> Result<()> {
        if let Some(ref renderer) = self.renderer {
            let ctx = self.render_context();
            for line in lines {
                renderer.render_line(line, w, &ctx)?;
                writeln!(w)?;
            }
            w.flush()?;
        }
        Ok(())
    }

    /// Finalize the current command's output.
    ///
    /// For full-input renderers this is where the actual rendering happens.
    /// For buffering state (not enough lines for detection), we try detection
    /// with whatever we have.
    pub fn finish(&mut self, w: &mut impl Write) -> Result<()> {
        match self.state {
            // Line-by-line renderer already flushed incrementally.
            State::Idle | State::Passthrough | State::Rendering => {}

            State::Buffering => {
                // Didn't collect enough lines — try detection anyway.
                let format = detect::detect_format(&self.clean_lines, None);
                if matches!(format, Format::Plain) {
                    w.write_all(&self.raw_buffer)?;
                } else {
                    self.render_all(format, w)?;
                }
                w.flush()?;
            }

            State::RenderingFull => {
                if let Some(ref renderer) = self.renderer {
                    let ctx = self.render_context();
                    let all = self.clean_lines.join("\n");
                    renderer.render_all(&all, w, &ctx)?;
                    writeln!(w)?;
                    w.flush()?;
                }
            }
        }

        self.reset();
        Ok(())
    }

    /// Discard buffered output and reset to idle (e.g. on alt-screen enter).
    pub fn abort(&mut self, w: &mut impl Write) -> Result<()> {
        if !self.raw_buffer.is_empty() {
            w.write_all(&self.raw_buffer)?;
            w.flush()?;
        }
        self.reset();
        Ok(())
    }

    // -- queries --------------------------------------------------------------

    pub fn is_active(&self) -> bool {
        self.state != State::Idle
    }

    pub fn is_passthrough(&self) -> bool {
        self.state == State::Passthrough
    }

    pub fn is_rendering(&self) -> bool {
        self.state == State::Rendering
    }

    // -- internal -------------------------------------------------------------

    const fn render_context(&self) -> RenderContext<'_> {
        RenderContext {
            terminal: &self.terminal,
            theme: self.theme,
            level_filter: self.level_filter,
            ascii: self.ascii,
        }
    }

    fn render_all(&self, format: Format, w: &mut impl Write) -> Result<()> {
        let renderer = render::renderer_for(format);
        let ctx = self.render_context();
        if renderer.wants_full_input() {
            let all = self.clean_lines.join("\n");
            renderer.render_all(&all, w, &ctx)?;
            writeln!(w)?;
        } else {
            for line in &self.clean_lines {
                renderer.render_line(line, w, &ctx)?;
                writeln!(w)?;
            }
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.state = State::Idle;
        self.buffering_start = None;
        self.raw_buffer.clear();
        self.clean_lines.clear();
        self.renderer = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_beautifier() -> OutputBeautifier<'static> {
        // Use a leaked theme to get a &'static reference — fine in tests.
        let theme: &'static Theme = Box::leak(Box::new(Theme::by_name("default")));
        OutputBeautifier::new(theme, None, false)
    }

    // -- State queries --------------------------------------------------------

    #[test]
    fn idle_on_creation() {
        let b = make_beautifier();
        assert!(!b.is_active());
        assert!(!b.is_passthrough());
        assert!(!b.is_rendering());
    }

    #[test]
    fn active_after_start() {
        let mut b = make_beautifier();
        b.start();
        assert!(b.is_active());
        assert!(!b.is_passthrough());
        assert!(!b.is_rendering());
    }

    #[test]
    fn idle_after_finish() {
        let mut b = make_beautifier();
        let mut out = Vec::new();
        b.start();
        b.finish(&mut out).unwrap();
        assert!(!b.is_active());
    }

    // -- Passthrough for plain text -------------------------------------------

    #[test]
    fn plain_text_falls_through_to_passthrough() {
        let mut b = make_beautifier();
        let mut out = Vec::new();

        b.start();

        // Feed enough plain text lines to trigger detection.
        let lines: Vec<String> = (0..DETECTION_BUFFER_SIZE)
            .map(|i| format!("plain line {i}"))
            .collect();
        let raw = lines.join("\n").into_bytes();
        b.feed_raw(&raw);
        b.feed_lines(lines);

        assert!(b.should_detect());
        b.detect_and_render(&mut out).unwrap();

        assert!(b.is_passthrough());
        // Raw bytes should have been flushed to output.
        assert!(!out.is_empty());
    }

    #[test]
    fn finish_while_buffering_plain_dumps_raw() {
        let mut b = make_beautifier();
        let mut out = Vec::new();

        b.start();
        let raw = b"some raw bytes";
        b.feed_raw(raw);
        b.feed_lines(vec!["some raw bytes".to_owned()]);

        // Finish before reaching DETECTION_BUFFER_SIZE — still tries detection.
        b.finish(&mut out).unwrap();
        assert_eq!(out, raw);
    }

    // -- JSON detection triggers rendering ------------------------------------

    #[test]
    fn json_detected_and_rendered() {
        let mut b = make_beautifier();
        let mut out = Vec::new();

        b.start();

        // Feed valid JSON lines — format detection should pick up JSON.
        let json_str = r#"{"name": "prezzy", "version": "0.1.0"}"#;
        let lines: Vec<String> = std::iter::repeat_n(json_str.to_owned(), DETECTION_BUFFER_SIZE)
            .collect();
        let raw = lines.join("\n").into_bytes();
        b.feed_raw(&raw);
        b.feed_lines(lines);

        assert!(b.should_detect());
        b.detect_and_render(&mut out).unwrap();

        // JSON renderer wants full input, so state should be RenderingFull.
        // (Not Passthrough, and not line-by-line Rendering.)
        assert!(b.is_active());
        assert!(!b.is_passthrough());
    }

    // -- Buffer limits --------------------------------------------------------

    #[test]
    fn raw_buffer_overflow_signals_caller() {
        let mut b = make_beautifier();
        b.start();

        let big_chunk = vec![0u8; MAX_RAW_BUFFER + 1];
        let overflow = b.feed_raw(&big_chunk);
        assert!(overflow);
    }

    #[test]
    fn over_limit_raw() {
        let mut b = make_beautifier();
        b.start();
        b.feed_raw(&vec![0u8; MAX_RAW_BUFFER + 1]);
        assert!(b.over_limit());
    }

    #[test]
    fn over_limit_clean_lines() {
        let mut b = make_beautifier();
        b.start();
        let lines: Vec<String> = (0..MAX_CLEAN_LINES + 1).map(|i| format!("{i}")).collect();
        b.feed_lines(lines);
        assert!(b.over_limit());
    }

    #[test]
    fn force_passthrough_flushes_and_clears() {
        let mut b = make_beautifier();
        let mut out = Vec::new();

        b.start();
        b.feed_raw(b"overflow data");
        b.feed_lines(vec!["overflow data".into()]);
        b.force_passthrough(&mut out).unwrap();

        assert!(b.is_passthrough());
        assert_eq!(out, b"overflow data");
    }

    // -- Detection timeout ----------------------------------------------------

    #[test]
    fn should_detect_false_initially() {
        let mut b = make_beautifier();
        b.start();
        b.feed_lines(vec!["one line".into()]);
        // Not enough lines, not enough time.
        assert!(!b.should_detect());
    }

    #[test]
    fn should_detect_true_at_buffer_size() {
        let mut b = make_beautifier();
        b.start();
        let lines: Vec<String> = (0..DETECTION_BUFFER_SIZE).map(|i| format!("{i}")).collect();
        b.feed_lines(lines);
        assert!(b.should_detect());
    }

    #[test]
    fn should_detect_true_after_timeout() {
        let mut b = make_beautifier();
        b.start();
        b.feed_lines(vec!["just one line".into()]);

        // Manually backdate the start time to simulate elapsed timeout.
        b.buffering_start = Some(Instant::now() - DETECTION_TIMEOUT - Duration::from_millis(1));
        assert!(b.should_detect());
    }

    #[test]
    fn should_detect_false_when_not_buffering() {
        let b = make_beautifier();
        // Not started — Idle state.
        assert!(!b.should_detect());
    }

    // -- Abort (alt-screen) ---------------------------------------------------

    #[test]
    fn abort_flushes_raw_and_resets() {
        let mut b = make_beautifier();
        let mut out = Vec::new();

        b.start();
        b.feed_raw(b"partial output");
        b.abort(&mut out).unwrap();

        assert!(!b.is_active());
        assert_eq!(out, b"partial output");
    }

    // -- Reuse across commands ------------------------------------------------

    #[test]
    fn reusable_across_commands() {
        let mut b = make_beautifier();
        let mut out = Vec::new();

        // First command.
        b.start();
        b.feed_raw(b"cmd1 output");
        b.feed_lines(vec!["cmd1 output".into()]);
        b.finish(&mut out).unwrap();

        // Second command — should start clean.
        out.clear();
        b.start();
        b.feed_raw(b"cmd2 output");
        b.feed_lines(vec!["cmd2 output".into()]);
        b.finish(&mut out).unwrap();

        assert_eq!(out, b"cmd2 output");
    }

    // -- feed_raw ignored in wrong states -------------------------------------

    #[test]
    fn feed_raw_noop_when_idle() {
        let mut b = make_beautifier();
        let overflow = b.feed_raw(b"ignored");
        assert!(!overflow);
    }

    #[test]
    fn feed_raw_noop_when_passthrough() {
        let mut b = make_beautifier();
        let mut out = Vec::new();
        b.start();
        b.force_passthrough(&mut out).unwrap();
        let overflow = b.feed_raw(b"ignored");
        assert!(!overflow);
    }

    #[test]
    fn feed_lines_noop_when_idle() {
        let mut b = make_beautifier();
        // Not calling start() — Idle state should silently ignore lines.
        b.feed_lines(vec!["ignored".into()]);
        assert!(!b.is_active());
    }
}
