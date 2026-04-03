//! PTY creation, shell detection, and child process spawning.

use std::path::PathBuf;

use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Detect the user's preferred shell from the environment.
pub fn detect_shell() -> String {
    if let Ok(shell) = std::env::var("SHELL") {
        // On Windows, MSYS/Cygwin sets $SHELL to /usr/bin/bash which isn't
        // a valid Windows path for ConPTY. Try the bare name instead.
        if cfg!(windows) && shell.starts_with('/') {
            let basename = shell_basename(&shell);
            if command_exists(&basename) {
                return basename;
            }
            // Fall through to Windows defaults.
        } else {
            return shell;
        }
    }

    if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
    } else {
        "/bin/sh".into()
    }
}

/// Extract the shell name from a full path (e.g. `/usr/bin/zsh` → `zsh`).
///
/// Handles both `/` and `\` separators on all platforms so that Windows-style
/// paths work correctly when tested on Linux CI.
pub fn shell_basename(path: &str) -> String {
    let name = path
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or("sh");
    // Strip extension (e.g. "cmd.exe" → "cmd").
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_lowercase(),
        _ => name.to_lowercase(),
    }
}

/// Everything needed to manage the PTY session lifetime.
pub struct PtySession {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    /// Temp file/dir to clean up when the session ends.
    cleanup_path: Option<PathBuf>,
}

impl Drop for PtySession {
    fn drop(&mut self) {
        super::inject::cleanup(self.cleanup_path.as_ref());
    }
}

/// Open a PTY and spawn the user's shell inside it.
///
/// The returned [`PtySession`] owns the master, child process, and any temp
/// files created for shell integration. Temp files are cleaned up on drop.
pub fn spawn_shell(
    shell_path: &str,
    shell_name: &str,
    cols: u16,
    rows: u16,
    passthrough: bool,
) -> Result<PtySession> {
    let pty_system = native_pty_system();

    let size = PtySize {
        rows: rows.max(2),
        cols: cols.max(2),
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = pty_system
        .openpty(size)
        .context("failed to open PTY pair")?;

    let mut cmd = CommandBuilder::new(shell_path);

    // portable-pty clears the environment when any env var is set,
    // so we must explicitly inherit everything from the parent.
    for (key, value) in std::env::vars_os() {
        cmd.env(key, value);
    }

    // Ensure a sensible TERM is set.
    if std::env::var_os("TERM").is_none() {
        cmd.env("TERM", "xterm-256color");
    }

    // Mark this as a prezzy shell session (for nested-session detection).
    cmd.env("PREZZY_SHELL", "1");

    // Inject OSC 133 shell integration markers (skip in passthrough mode).
    let cleanup_path = if passthrough {
        None
    } else {
        super::inject::prepare_command(&mut cmd, shell_name)?
    };

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("failed to spawn shell process")?;

    // Drop the slave so the master gets EOF when the child exits.
    drop(pair.slave);

    Ok(PtySession {
        master: pair.master,
        child,
        cleanup_path,
    })
}

/// Check whether a command is reachable on PATH without executing it.
///
/// Uses the platform's native lookup (`where` on Windows, `command -v` on Unix)
/// instead of running the target binary, which avoids side effects (some shells
/// source profile files even with `--help`).
fn command_exists(name: &str) -> bool {
    let (probe, args): (&str, &[&str]) = if cfg!(windows) {
        ("where.exe", &[name])
    } else {
        ("sh", &["-c", &format!("command -v '{name}'")])
    };
    std::process::Command::new(probe)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- shell_basename -------------------------------------------------------

    #[test]
    fn basename_unix_path() {
        assert_eq!(shell_basename("/usr/bin/zsh"), "zsh");
    }

    #[test]
    fn basename_unix_nested() {
        assert_eq!(shell_basename("/usr/local/bin/fish"), "fish");
    }

    #[test]
    fn basename_bare_name() {
        assert_eq!(shell_basename("bash"), "bash");
    }

    #[test]
    fn basename_windows_path() {
        assert_eq!(shell_basename("C:\\Windows\\System32\\cmd.exe"), "cmd");
    }

    #[test]
    fn basename_uppercased_normalized() {
        assert_eq!(shell_basename("/usr/bin/ZSH"), "zsh");
    }

    #[test]
    fn basename_empty_falls_back() {
        // Path::file_stem("") returns None → fallback "sh".
        assert_eq!(shell_basename(""), "sh");
    }

    #[test]
    fn basename_trailing_slash() {
        // Unusual but shouldn't panic.
        let result = shell_basename("/usr/bin/");
        // Path::file_stem for trailing slash returns "bin" on most platforms.
        assert!(!result.is_empty());
    }

    // -- detect_shell (environment-dependent) ---------------------------------

    #[test]
    fn detect_shell_returns_nonempty() {
        // Regardless of environment, should always return something usable.
        let shell = detect_shell();
        assert!(!shell.is_empty());
    }

    // -- command_exists -------------------------------------------------------

    #[test]
    fn command_exists_finds_common_tools() {
        // `where.exe` on Windows, `sh` on Unix — these always exist.
        if cfg!(windows) {
            assert!(command_exists("cmd.exe"));
        } else {
            assert!(command_exists("sh"));
        }
    }

    #[test]
    fn command_exists_rejects_nonexistent() {
        assert!(!command_exists("__prezzy_nonexistent_binary_12345__"));
    }
}
