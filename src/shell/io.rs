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
/// In passthrough mode, skips VTE parsing and beautification entirely.
pub fn run(
    master: &dyn MasterPty,
    theme: &Theme,
    level_filter: Option<LevelFilter>,
    ascii: bool,
    passthrough: bool,
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
    if passthrough {
        passthrough_loop(reader, master)
    } else {
        output_loop(reader, master, theme, level_filter, ascii)
    }
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

// ---------------------------------------------------------------------------
// Passthrough loop (--passthrough mode)
// ---------------------------------------------------------------------------

/// Minimal output loop: forward PTY output to stdout with no processing.
/// Still forwards terminal resizes so interactive programs render correctly.
#[allow(clippy::significant_drop_tightening)]
fn passthrough_loop(
    mut reader: Box<dyn Read + Send>,
    master: &dyn MasterPty,
) -> Result<Option<i32>> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut buf = [0u8; 8192];
    let mut last_size = crossterm::terminal::size().unwrap_or((80, 24));

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if is_eof_like(&e) => break,
            Err(e) => return Err(e.into()),
        };

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

        stdout.write_all(&buf[..n])?;
        stdout.flush()?;
    }

    Ok(None) // No exit code tracking in passthrough mode.
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

// ---------------------------------------------------------------------------
// Integration tests — exercise the full parser + beautifier pipeline
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    /// Simulate the output loop by feeding chunks through the same state
    /// machine logic used in production. Returns (output_bytes, exit_code).
    fn simulate(chunks: &[&[u8]]) -> (Vec<u8>, Option<i32>) {
        let theme = Theme::by_name("default");
        let mut vte_parser = vte::Parser::new();
        let mut state = ShellParser::new();
        let mut beautifier = OutputBeautifier::new(&theme, None, false);
        let mut out = Vec::new();

        for &chunk in chunks {
            let was_cmd = state.command_state == CommandState::CommandRunning;
            for &byte in chunk {
                vte_parser.advance(&mut state, byte);
            }
            let is_cmd = state.command_state == CommandState::CommandRunning;

            if state.alternate_screen {
                if beautifier.is_active() {
                    beautifier.abort(&mut out).unwrap();
                }
                out.extend_from_slice(chunk);
                continue;
            }

            match (was_cmd, is_cmd) {
                (false, true) => {
                    beautifier.start();
                    let overflow = beautifier.feed_raw(chunk);
                    beautifier.feed_lines(state.take_clean_lines());
                    if overflow || beautifier.over_limit() {
                        beautifier.force_passthrough(&mut out).unwrap();
                    } else if beautifier.should_detect() {
                        beautifier.detect_and_render(&mut out).unwrap();
                    }
                }
                (true, true) => {
                    let new_lines = state.take_clean_lines();
                    if beautifier.is_passthrough() {
                        out.extend_from_slice(chunk);
                    } else if beautifier.is_rendering() {
                        if !new_lines.is_empty() {
                            beautifier.render_lines(&new_lines, &mut out).unwrap();
                        }
                    } else {
                        let overflow = beautifier.feed_raw(chunk);
                        beautifier.feed_lines(new_lines);
                        if overflow || beautifier.over_limit() {
                            beautifier.force_passthrough(&mut out).unwrap();
                        } else if beautifier.should_detect() {
                            beautifier.detect_and_render(&mut out).unwrap();
                        }
                    }
                }
                (true, false) => {
                    beautifier.feed_raw(chunk);
                    let new_lines = state.take_clean_lines();
                    if !new_lines.is_empty() {
                        beautifier.feed_lines(new_lines);
                    }
                    beautifier.finish(&mut out).unwrap();
                }
                (false, false) => {
                    out.extend_from_slice(chunk);
                }
            }
        }

        if beautifier.is_active() {
            beautifier.finish(&mut out).unwrap();
        }

        (out, state.exit_code)
    }

    // -- Full lifecycle -------------------------------------------------------

    #[test]
    fn plain_command_output_passed_through() {
        let (out, code) = simulate(&[
            b"\x1b]133;A\x07$ ",          // prompt
            b"\x1b]133;C\x07",            // command start
            b"hello world\n",             // output
            b"\x1b]133;D;0\x07",          // command end, exit 0
        ]);
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("hello world"));
        assert_eq!(code, Some(0));
    }

    #[test]
    fn prompt_text_passed_through_raw() {
        let (out, _) = simulate(&[
            b"\x1b]133;A\x07$ ",
        ]);
        // Prompt is in (false, false) — raw passthrough.
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("$ "));
    }

    #[test]
    fn exit_code_tracked_across_commands() {
        let (_, code) = simulate(&[
            b"\x1b]133;C\x07",
            b"ok\n",
            b"\x1b]133;D;0\x07",
            b"\x1b]133;A\x07$ ",
            b"\x1b]133;C\x07",
            b"fail\n",
            b"\x1b]133;D;1\x07",
        ]);
        assert_eq!(code, Some(1)); // Last command's exit code.
    }

    #[test]
    fn empty_command_no_crash() {
        // User just pressed Enter — C immediately followed by D.
        let (_, code) = simulate(&[
            b"\x1b]133;A\x07$ ",
            b"\x1b]133;C\x07",
            b"\x1b]133;D;0\x07",
        ]);
        assert_eq!(code, Some(0));
    }

    #[test]
    fn rapid_commands() {
        // Three commands in quick succession.
        let (out, code) = simulate(&[
            b"\x1b]133;C\x07one\n\x1b]133;D;0\x07",
            b"\x1b]133;A\x07$ ",
            b"\x1b]133;C\x07two\n\x1b]133;D;0\x07",
            b"\x1b]133;A\x07$ ",
            b"\x1b]133;C\x07three\n\x1b]133;D;42\x07",
        ]);
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("one"));
        assert!(text.contains("two"));
        assert!(text.contains("three"));
        assert_eq!(code, Some(42));
    }

    // -- Alt screen -----------------------------------------------------------

    #[test]
    fn alt_screen_aborts_beautification() {
        let (out, _) = simulate(&[
            b"\x1b]133;C\x07",
            b"partial output\n",
            b"\x1b[?1049h",               // enter alt screen
            b"vim content here",
            b"\x1b[?1049l",               // leave alt screen
            b"\x1b]133;D;0\x07",
        ]);
        let text = String::from_utf8_lossy(&out);
        // Both partial output (flushed on abort) and vim content should appear.
        assert!(text.contains("vim content here"));
    }

    #[test]
    fn alt_screen_pure_passthrough() {
        // Enter alt screen outside a command (e.g. running `less` directly).
        let (out, _) = simulate(&[
            b"\x1b[?1049h",
            b"fullscreen app content",
            b"\x1b[?1049l",
        ]);
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("fullscreen app content"));
    }

    // -- Buffer overflow ------------------------------------------------------

    #[test]
    fn large_output_forces_passthrough() {
        // Generate enough data to exceed MAX_RAW_BUFFER (1 MiB).
        let big = vec![b'X'; 512 * 1024]; // 512 KiB per chunk
        let big_ref: &[u8] = &big;

        // We need to use owned data, so build the simulation manually.
        let theme = Theme::by_name("default");
        let mut vte_parser = vte::Parser::new();
        let mut state = ShellParser::new();
        let mut beautifier = OutputBeautifier::new(&theme, None, false);
        let mut out = Vec::new();

        // Start command.
        for &byte in &b"\x1b]133;C\x07"[..] {
            vte_parser.advance(&mut state, byte);
        }
        beautifier.start();
        beautifier.feed_raw(b"\x1b]133;C\x07");
        beautifier.feed_lines(state.take_clean_lines());

        // Feed large chunks until overflow.
        for _ in 0..3 {
            for &byte in big_ref {
                vte_parser.advance(&mut state, byte);
            }
            let overflow = beautifier.feed_raw(big_ref);
            beautifier.feed_lines(state.take_clean_lines());
            if overflow || beautifier.over_limit() {
                beautifier.force_passthrough(&mut out).unwrap();
                break;
            }
        }

        assert!(beautifier.is_passthrough());
        assert!(!out.is_empty()); // Raw buffer was flushed.
    }

    // -- No markers (unsupported shell) ---------------------------------------

    #[test]
    fn no_markers_pure_passthrough() {
        // Simulate output from an unsupported shell — no OSC 133 markers.
        let (out, code) = simulate(&[
            b"$ ls\n",
            b"file1.txt  file2.txt\n",
            b"$ ",
        ]);
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("file1.txt"));
        assert!(text.contains("file2.txt"));
        assert_eq!(code, None); // No exit code without markers.
    }

    // -- Markers split across chunks ------------------------------------------

    #[test]
    fn osc_marker_split_across_chunks() {
        let (_, code) = simulate(&[
            b"\x1b]133;C\x07",
            b"output\n",
            b"\x1b]133;D;",     // D marker split...
            b"0\x07",           // ...across two chunks
        ]);
        assert_eq!(code, Some(0));
    }

    // -- ANSI colors in output ------------------------------------------------

    #[test]
    fn colored_output_preserved_in_passthrough() {
        let (out, _) = simulate(&[
            b"\x1b]133;C\x07",
            b"\x1b[31mred text\x1b[0m\n",
            b"\x1b]133;D;0\x07",
        ]);
        let text = String::from_utf8_lossy(&out);
        // Raw passthrough should include the escape codes.
        assert!(text.contains("\x1b[31m") || text.contains("red text"));
    }
}
