//! I/O thread management and the core output processing loop.
//!
//! Thread model (no mutexes, no shared mutable state):
//!
//! ```text
//! Main thread           Input thread          Output thread (current)
//! ───────────           ────────────          ───────────────────────
//! spawn PTY
//! spawn input thread
//!                       loop {                loop {
//!                         read stdin            read PTY master
//!                         write to PTY          VTE parse → state
//!                       }                       beautify or passthrough
//!                                               write to stdout
//!                                             }
//! join output
//! restore terminal
//! ```

use std::io::{self, Read, Write};

use anyhow::{Context, Result};
use portable_pty::MasterPty;

use crate::render::LevelFilter;
use crate::theme::Theme;

use super::beautify::OutputBeautifier;
use super::parser::{CommandState, ShellParser};

/// RAII guard that enables terminal raw mode on creation and disables it
/// on drop, ensuring the terminal is always restored even on panic.
pub struct RawModeGuard {
    _private: (),
}

impl RawModeGuard {
    pub fn enable() -> Result<Self> {
        crossterm::terminal::enable_raw_mode().context("enable raw mode")?;
        Ok(Self { _private: () })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

/// Spawn I/O threads and run the output processing loop.
///
/// Returns the last exit code reported by the child shell (via OSC 133;D).
pub fn run(
    master: &dyn MasterPty,
    theme: &Theme,
    level_filter: Option<LevelFilter>,
    ascii: bool,
) -> Result<Option<i32>> {
    let reader = master
        .try_clone_reader()
        .context("clone PTY reader")?;
    let writer = master
        .take_writer()
        .context("take PTY writer")?;

    // Input thread: stdin → PTY master (detached — will die with process).
    let _input = std::thread::Builder::new()
        .name("prezzy-input".into())
        .spawn(move || {
            let _ = input_loop(writer);
        })?;

    // Output loop runs on the current thread so we can borrow `theme`.
    // We also pass `master` so the loop can forward terminal resizes.
    output_loop(reader, master, theme, level_filter, ascii)
}

// ---------------------------------------------------------------------------
// Input thread
// ---------------------------------------------------------------------------

/// Forward every byte from stdin to the PTY master.
///
/// Uses a 1 KiB buffer — large enough for paste bursts, small enough to
/// keep individual keystroke latency imperceptible.
#[allow(clippy::significant_drop_tightening)] // Lock must live across reads.
fn input_loop(mut writer: Box<dyn Write + Send>) -> Result<()> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut buf = [0u8; 1024];

    loop {
        let n = stdin.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        writer.flush()?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Output loop
// ---------------------------------------------------------------------------

/// Read from the PTY master, parse ANSI escapes for state tracking, and
/// either beautify or pass through to stdout.
#[allow(clippy::significant_drop_tightening)] // stdout lock must live across loop.
fn output_loop(
    mut reader: Box<dyn Read + Send>,
    master: &dyn MasterPty,
    theme: &Theme,
    level_filter: Option<LevelFilter>,
    ascii: bool,
) -> Result<Option<i32>> {
    let stdout = io::stdout();
    let mut stdout = io::BufWriter::new(stdout.lock());
    let mut vte_parser = vte::Parser::new();
    let mut state = ShellParser::new();
    let mut beautifier = OutputBeautifier::new(theme, level_filter, ascii);
    let mut buf = [0u8; 8192];

    // Track terminal size so we can detect resizes and forward them to the PTY.
    let mut last_size = crossterm::terminal::size().unwrap_or((80, 24));

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,                  // EOF — child exited
            Ok(n) => n,
            Err(e) if is_eof_like(&e) => break,
            Err(e) => return Err(e.into()),
        };
        let chunk = &buf[..n];

        // Check for terminal resize and forward to PTY.
        if let Ok(new_size) = crossterm::terminal::size() {
            if new_size != last_size {
                last_size = new_size;
                let _ = master.resize(portable_pty::PtySize {
                    cols: new_size.0,
                    rows: new_size.1,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
        }

        // Snapshot state before parsing.
        let was_cmd = state.command_state == CommandState::CommandRunning;

        // Feed every byte through VTE for state tracking.
        // This also collects clean text when a command is running.
        for &byte in chunk {
            vte_parser.advance(&mut state, byte);
        }

        let is_cmd = state.command_state == CommandState::CommandRunning;

        // --- Alternate screen: always raw passthrough ---
        if state.alternate_screen {
            if beautifier.is_active() {
                beautifier.abort(&mut stdout)?;
            }
            stdout.write_all(chunk)?;
            stdout.flush()?;
            continue;
        }

        // --- State machine transitions ---
        match (was_cmd, is_cmd) {
            // Command just started executing.
            (false, true) => {
                beautifier.start();
                let overflow = beautifier.feed_raw(chunk);
                beautifier.feed_lines(state.take_clean_lines());
                if overflow || beautifier.over_limit() {
                    beautifier.force_passthrough(&mut stdout)?;
                } else if beautifier.should_detect() {
                    beautifier.detect_and_render(&mut stdout)?;
                }
            }

            // Command is still running.
            (true, true) => {
                let new_lines = state.take_clean_lines();

                if beautifier.is_passthrough() {
                    // Format was Plain — forward raw.
                    stdout.write_all(chunk)?;
                    stdout.flush()?;
                } else if beautifier.is_rendering() {
                    // Line-by-line renderer active — render new lines.
                    if !new_lines.is_empty() {
                        beautifier.render_lines(&new_lines, &mut stdout)?;
                    }
                } else {
                    // Still buffering for detection.
                    let overflow = beautifier.feed_raw(chunk);
                    beautifier.feed_lines(new_lines);

                    if overflow || beautifier.over_limit() {
                        // Buffer too large — abort beautification, passthrough.
                        beautifier.force_passthrough(&mut stdout)?;
                    } else if beautifier.should_detect() {
                        beautifier.detect_and_render(&mut stdout)?;
                    }
                }
            }

            // Command just finished.
            (true, false) => {
                // Feed remaining raw bytes — needed if finish() falls back to
                // dumping raw_buffer (Plain format or Buffering state).
                beautifier.feed_raw(chunk);
                let new_lines = state.take_clean_lines();
                if !new_lines.is_empty() {
                    beautifier.feed_lines(new_lines);
                }
                beautifier.finish(&mut stdout)?;
            }

            // No command running — normal passthrough.
            (false, false) => {
                stdout.write_all(chunk)?;
                stdout.flush()?;
            }
        }
    }

    // Flush anything left from an interrupted command.
    if beautifier.is_active() {
        beautifier.finish(&mut stdout)?;
    }
    stdout.flush()?;

    Ok(state.exit_code)
}

/// Returns `true` for I/O errors that indicate the other end hung up.
fn is_eof_like(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::UnexpectedEof
    )
}
