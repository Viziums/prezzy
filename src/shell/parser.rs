//! ANSI escape sequence parser for shell mode.
//!
//! Wraps the `vte` crate (Alacritty's parser) to track two things:
//!
//! 1. **Alternate screen** — When programs like vim, htop, or less activate
//!    the alternate screen buffer (CSI ?1049h), we pass output through raw
//!    so those programs aren't disturbed.
//!
//! 2. **Command boundaries** — OSC 133 markers injected by our shell
//!    integration scripts tell us when a command starts executing and when
//!    it finishes, so we can buffer output per-command for format detection.
//!
//! The parser also collects "clean text" (printable characters only, no ANSI)
//! while a command is running, which is what the format detectors analyse.

/// Tracks the shell lifecycle as driven by OSC 133 markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandState {
    /// No command running — between prompts or before the first prompt.
    Idle,
    /// The shell prompt is being displayed.
    PromptShowing,
    /// A command is executing and producing output.
    CommandRunning,
}

/// Maximum length for a single clean text line before we stop collecting.
/// Prevents OOM on commands that output huge lines without newlines.
const MAX_LINE_LEN: usize = 64 * 1024; // 64 KiB

/// VTE-based ANSI parser tracking shell state and collecting clean text.
pub struct ShellParser {
    // -- state tracking -------------------------------------------------------
    pub alternate_screen: bool,
    pub command_state: CommandState,
    /// Exit code reported by the most recent OSC 133;D marker.
    pub exit_code: Option<i32>,

    // -- command metadata (for history) ---------------------------------------
    /// Command text from the most recent OSC 133;E marker.
    pub command_text: Option<String>,
    /// Working directory from the most recent OSC 133;W marker.
    pub command_cwd: Option<String>,

    // -- clean text collection ------------------------------------------------
    /// Complete lines of clean text (newline-delimited) collected while a
    /// command is running. Used for format detection.
    clean_lines: Vec<String>,
    /// The current incomplete line being built.
    current_line: String,
}

impl ShellParser {
    pub const fn new() -> Self {
        Self {
            alternate_screen: false,
            command_state: CommandState::Idle,
            exit_code: None,
            command_text: None,
            command_cwd: None,
            clean_lines: Vec::new(),
            current_line: String::new(),
        }
    }

    /// Take all complete clean lines accumulated so far, leaving the
    /// internal buffer empty.
    pub fn take_clean_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.clean_lines)
    }

    /// Flush any partial line into `clean_lines`.
    pub fn flush_partial_line(&mut self) {
        if !self.current_line.is_empty() {
            self.clean_lines
                .push(std::mem::take(&mut self.current_line));
        }
    }

    /// Take the command text reported by the E marker, if any.
    #[allow(clippy::missing_const_for_fn)] // Option::take() isn't const-stable across toolchains.
    pub fn take_command_text(&mut self) -> Option<String> {
        self.command_text.take()
    }

    /// Take the working directory reported by the W marker, if any.
    #[allow(clippy::missing_const_for_fn)]
    pub fn take_command_cwd(&mut self) -> Option<String> {
        self.command_cwd.take()
    }

    /// Reset collection state for a new command.
    fn begin_command(&mut self) {
        self.clean_lines.clear();
        self.current_line.clear();
    }

    /// Whether we are currently collecting clean text.
    fn collecting(&self) -> bool {
        self.command_state == CommandState::CommandRunning && !self.alternate_screen
    }
}

impl vte::Perform for ShellParser {
    /// A printable character.
    fn print(&mut self, c: char) {
        if self.collecting() && self.current_line.len() < MAX_LINE_LEN {
            self.current_line.push(c);
        }
    }

    /// A C0/C1 control character (newline, tab, etc.).
    fn execute(&mut self, byte: u8) {
        if self.collecting() {
            match byte {
                b'\n' => {
                    self.clean_lines
                        .push(std::mem::take(&mut self.current_line));
                }
                b'\t' => self.current_line.push('\t'),
                // Ignore \r and other controls for detection purposes.
                _ => {}
            }
        }
    }

    /// A CSI (Control Sequence Introducer) escape sequence.
    ///
    /// We care about alternate screen toggle:
    ///   CSI ? 1049 h  — enter alternate screen
    ///   CSI ? 1049 l  — leave alternate screen
    ///   (also ?47 and ?1047 for older terminals)
    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        if intermediates == [b'?'] {
            for param in params {
                if param.len() == 1 && matches!(param[0], 1049 | 47 | 1047) {
                    match action {
                        'h' => self.alternate_screen = true,
                        'l' => self.alternate_screen = false,
                        _ => {}
                    }
                }
            }
        }
    }

    /// An OSC (Operating System Command) escape sequence.
    ///
    /// We care about OSC 133 (semantic prompts):
    ///   OSC 133 ; A ST — prompt start
    ///   OSC 133 ; C ST — command executing / output begins
    ///   OSC 133 ; D [; `exit_code`] ST — command finished
    ///   OSC 133 ; E [; `command`]  ST — command text (prezzy extension)
    ///   OSC 133 ; W [; `cwd`]     ST — working directory (prezzy extension)
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() < 2 || params[0] != b"133" {
            return;
        }

        match params[1] {
            b"A" => {
                // Prompt showing — previous command (if any) is done.
                self.flush_partial_line();
                self.command_state = CommandState::PromptShowing;
            }
            b"C" => {
                // Command started executing.
                self.command_state = CommandState::CommandRunning;
                self.begin_command();
            }
            b"D" => {
                // Command finished.
                self.flush_partial_line();
                self.command_state = CommandState::Idle;

                // Parse exit code: OSC 133 ; D ; <code> ST
                if params.len() >= 3 {
                    if let Ok(s) = std::str::from_utf8(params[2]) {
                        self.exit_code = s.parse::<i32>().ok();
                    }
                } else {
                    // Bare D without exit code -- assume success.
                    self.exit_code = Some(0);
                }
            }
            b"E" => {
                // Command text — join remaining params with ";" since
                // VTE splits on ";" and the command may contain semicolons.
                if params.len() >= 3 {
                    self.command_text = Some(join_params(&params[2..]));
                }
            }
            b"W" => {
                // Working directory — same join logic for paths with ";".
                if params.len() >= 3 {
                    self.command_cwd = Some(join_params(&params[2..]));
                }
            }
            _ => {}
        }
    }

    // The remaining vte::Perform methods have no-op defaults, which is fine
    // — we don't need to track DCS, ESC dispatches, or hooks for our
    // use-case.
}

/// Reconstruct a string from OSC params that were split on ";".
fn join_params(parts: &[&[u8]]) -> String {
    parts
        .iter()
        .filter_map(|p| std::str::from_utf8(p).ok())
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed a byte slice through VTE into our parser.
    fn feed(parser: &mut ShellParser, vte: &mut vte::Parser, input: &[u8]) {
        for &byte in input {
            vte.advance(parser, byte);
        }
    }

    // -- OSC 133 state transitions --------------------------------------------

    #[test]
    fn osc_133_full_lifecycle() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        assert_eq!(p.command_state, CommandState::Idle);

        // Prompt shown: OSC 133;A BEL
        feed(&mut p, &mut vte, b"\x1b]133;A\x07");
        assert_eq!(p.command_state, CommandState::PromptShowing);

        // Command starts: OSC 133;C BEL
        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        assert_eq!(p.command_state, CommandState::CommandRunning);

        // Command output
        feed(&mut p, &mut vte, b"hello world\n");

        // Command finishes: OSC 133;D;0 BEL
        feed(&mut p, &mut vte, b"\x1b]133;D;0\x07");
        assert_eq!(p.command_state, CommandState::Idle);
        assert_eq!(p.exit_code, Some(0));
    }

    #[test]
    fn exit_code_parsed_correctly() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"\x1b]133;D;42\x07");
        assert_eq!(p.exit_code, Some(42));
    }

    #[test]
    fn exit_code_negative() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"\x1b]133;D;-1\x07");
        assert_eq!(p.exit_code, Some(-1));
    }

    #[test]
    fn exit_code_defaults_none_without_d_marker() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;A\x07");
        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        assert_eq!(p.exit_code, None);
    }

    #[test]
    fn exit_code_overwritten_by_subsequent_command() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        // First command exits 0.
        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"\x1b]133;D;0\x07");
        assert_eq!(p.exit_code, Some(0));

        // Second command exits 1.
        feed(&mut p, &mut vte, b"\x1b]133;A\x07");
        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"\x1b]133;D;1\x07");
        assert_eq!(p.exit_code, Some(1));
    }

    #[test]
    fn d_marker_without_exit_code_assumes_success() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"\x1b]133;D;5\x07");
        assert_eq!(p.exit_code, Some(5));

        // A bare D with no code field assumes success (0).
        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"\x1b]133;D\x07");
        assert_eq!(p.exit_code, Some(0));
    }

    // -- Alternate screen detection -------------------------------------------

    #[test]
    fn alternate_screen_1049() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        assert!(!p.alternate_screen);
        feed(&mut p, &mut vte, b"\x1b[?1049h");
        assert!(p.alternate_screen);
        feed(&mut p, &mut vte, b"\x1b[?1049l");
        assert!(!p.alternate_screen);
    }

    #[test]
    fn alternate_screen_47() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b[?47h");
        assert!(p.alternate_screen);
        feed(&mut p, &mut vte, b"\x1b[?47l");
        assert!(!p.alternate_screen);
    }

    #[test]
    fn alternate_screen_1047() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b[?1047h");
        assert!(p.alternate_screen);
        feed(&mut p, &mut vte, b"\x1b[?1047l");
        assert!(!p.alternate_screen);
    }

    #[test]
    fn unrelated_csi_does_not_toggle_alt_screen() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        // SGR (color), cursor movement — should not affect alt screen.
        feed(&mut p, &mut vte, b"\x1b[31m\x1b[2J\x1b[H");
        assert!(!p.alternate_screen);
    }

    // -- Clean text collection ------------------------------------------------

    #[test]
    fn collects_clean_text_during_command() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"line one\nline two\n");

        let lines = p.take_clean_lines();
        assert_eq!(lines, vec!["line one", "line two"]);
    }

    #[test]
    fn does_not_collect_outside_command() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        // Before any command — Idle state.
        feed(&mut p, &mut vte, b"some prompt text\n");
        assert!(p.take_clean_lines().is_empty());

        // During PromptShowing.
        feed(&mut p, &mut vte, b"\x1b]133;A\x07");
        feed(&mut p, &mut vte, b"$ \n");
        assert!(p.take_clean_lines().is_empty());
    }

    #[test]
    fn does_not_collect_during_alt_screen() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"\x1b[?1049h"); // enter alt screen
        feed(&mut p, &mut vte, b"vim content\n");
        assert!(p.take_clean_lines().is_empty());

        feed(&mut p, &mut vte, b"\x1b[?1049l"); // leave alt screen
        feed(&mut p, &mut vte, b"normal output\n");
        let lines = p.take_clean_lines();
        assert_eq!(lines, vec!["normal output"]);
    }

    #[test]
    fn strips_ansi_from_clean_text() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        // Red text: ESC[31m hello ESC[0m
        feed(&mut p, &mut vte, b"\x1b[31mhello\x1b[0m\n");

        let lines = p.take_clean_lines();
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn preserves_tabs_in_clean_text() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"key\tvalue\n");

        let lines = p.take_clean_lines();
        assert_eq!(lines, vec!["key\tvalue"]);
    }

    #[test]
    fn flush_partial_line() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"no newline at end");

        // Not yet in clean_lines (no \n).
        assert!(p.take_clean_lines().is_empty());

        // Flush forces partial line into clean_lines.
        p.flush_partial_line();
        let lines = p.take_clean_lines();
        assert_eq!(lines, vec!["no newline at end"]);
    }

    #[test]
    fn begin_command_resets_collection() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        // First command.
        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"first\n");

        // Second command — should clear stale lines.
        feed(&mut p, &mut vte, b"\x1b]133;D;0\x07");
        feed(&mut p, &mut vte, b"\x1b]133;A\x07");
        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"second\n");

        let lines = p.take_clean_lines();
        assert_eq!(lines, vec!["second"]);
    }

    #[test]
    fn max_line_length_enforced() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;C\x07");

        // Feed more than MAX_LINE_LEN bytes without a newline.
        let big_chunk = vec![b'A'; MAX_LINE_LEN + 1000];
        feed(&mut p, &mut vte, &big_chunk);
        feed(&mut p, &mut vte, b"\n");

        let lines = p.take_clean_lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), MAX_LINE_LEN);
    }

    // -- Split escape sequences across chunks ---------------------------------

    #[test]
    fn split_osc_across_chunks() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        // Split OSC 133;C across two feeds.
        feed(&mut p, &mut vte, b"\x1b]133;");
        assert_eq!(p.command_state, CommandState::Idle); // not yet complete
        feed(&mut p, &mut vte, b"C\x07");
        assert_eq!(p.command_state, CommandState::CommandRunning);
    }

    #[test]
    fn split_csi_across_chunks() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        // Split CSI ?1049h across two feeds.
        feed(&mut p, &mut vte, b"\x1b[?1049");
        assert!(!p.alternate_screen);
        feed(&mut p, &mut vte, b"h");
        assert!(p.alternate_screen);
    }

    // -- Ignores unrelated OSC sequences --------------------------------------

    // -- Command text (E marker) and CWD (W marker) ---------------------------

    #[test]
    fn e_marker_captures_command_text() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;E;echo hello\x07");
        assert_eq!(p.command_text.as_deref(), Some("echo hello"));
    }

    #[test]
    fn e_marker_with_semicolons_in_command() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        // Command: echo a; echo b — VTE splits on ";" so we rejoin.
        feed(&mut p, &mut vte, b"\x1b]133;E;echo a; echo b\x07");
        assert_eq!(p.command_text.as_deref(), Some("echo a; echo b"));
    }

    #[test]
    fn w_marker_captures_cwd() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;W;/home/user/project\x07");
        assert_eq!(p.command_cwd.as_deref(), Some("/home/user/project"));
    }

    #[test]
    fn take_command_text_clears() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;E;ls\x07");
        assert_eq!(p.take_command_text().as_deref(), Some("ls"));
        assert!(p.take_command_text().is_none()); // consumed
    }

    #[test]
    fn e_and_w_in_full_lifecycle() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        feed(&mut p, &mut vte, b"\x1b]133;A\x07");
        feed(&mut p, &mut vte, b"\x1b]133;E;git status\x07");
        feed(&mut p, &mut vte, b"\x1b]133;W;/repo\x07");
        feed(&mut p, &mut vte, b"\x1b]133;C\x07");
        feed(&mut p, &mut vte, b"output\n");
        feed(&mut p, &mut vte, b"\x1b]133;D;0\x07");

        assert_eq!(p.command_text.as_deref(), Some("git status"));
        assert_eq!(p.command_cwd.as_deref(), Some("/repo"));
        assert_eq!(p.exit_code, Some(0));
    }

    // -- Ignores unrelated OSC sequences --------------------------------------

    #[test]
    fn ignores_non_133_osc() {
        let mut p = ShellParser::new();
        let mut vte = vte::Parser::new();

        // OSC 0 (set window title) — should not affect state.
        feed(&mut p, &mut vte, b"\x1b]0;My Title\x07");
        assert_eq!(p.command_state, CommandState::Idle);

        // OSC 7 (current directory) — should not affect state.
        feed(&mut p, &mut vte, b"\x1b]7;file:///home/user\x07");
        assert_eq!(p.command_state, CommandState::Idle);
    }
}
