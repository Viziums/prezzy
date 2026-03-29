//! Shell mode: wrap the user's interactive shell in a PTY with automatic
//! output beautification.
//!
//! Launch with `prezzy shell`. Every command's output is buffered, its format
//! detected (JSON, diff, logs, …), and rendered with syntax highlighting —
//! transparently, without changing how the shell works.
//!
//! Programs that use the alternate screen (vim, htop, less) are passed
//! through completely unmodified.

mod beautify;
mod inject;
mod io;
mod parser;
mod pty;

use anyhow::{Result, bail};

use crate::cli::ShellArgs;
use crate::config::Config;
use crate::render::LevelFilter;
use crate::theme::Theme;

/// Entry point for `prezzy shell`.
pub fn run(args: &ShellArgs) -> Result<()> {
    // Guard against nested sessions.
    if std::env::var_os("PREZZY_SHELL").is_some() {
        bail!(
            "already inside a prezzy shell session (PREZZY_SHELL is set)\n\
             Tip: run `exit` first, or unset PREZZY_SHELL to override."
        );
    }

    // Install a panic hook that restores the terminal before printing the
    // panic message. Without this, a panic leaves the terminal in raw mode.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        default_hook(info);
    }));

    // Resolve configuration (CLI args > config file > defaults).
    let config = Config::load();
    let theme_name = resolve_theme(&args.theme, &config);
    let theme = Theme::by_name(&theme_name);
    let level_filter = args.level.as_deref().and_then(LevelFilter::parse);
    let ascii = args.ascii || config.ascii.unwrap_or(false);

    // Detect shell and terminal size.
    let shell_path = pty::detect_shell();
    let shell_name = pty::shell_basename(&shell_path);
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));

    eprintln!(
        "prezzy: launching {shell_name} in shell mode (beautification active)"
    );

    // Spawn child shell in a PTY. PtySession cleans up temp files on drop.
    let mut session = pty::spawn_shell(&shell_path, &shell_name, cols, rows)?;

    // Put the outer terminal into raw mode so keystrokes pass through.
    let raw_guard = io::RawModeGuard::enable()?;

    // Run I/O threads — blocks until the child shell exits.
    let exit_code = io::run(&*session.master, &theme, level_filter, ascii)?;

    // Restore terminal before printing anything.
    drop(raw_guard);

    // Remove our custom panic hook — no longer needed after raw mode is off.
    // This prevents hook accumulation if run() is ever called in non-exit contexts.
    let _ = std::panic::take_hook();

    // Reap the child process.
    let _ = session.child.wait();

    // PtySession drop cleans up temp init scripts.
    drop(session);

    // Exit with the shell's last reported exit code.
    std::process::exit(exit_code.unwrap_or(0));
}

fn resolve_theme(cli_theme: &str, config: &Config) -> String {
    if cli_theme != "default" {
        return cli_theme.to_owned();
    }
    config
        .theme
        .clone()
        .unwrap_or_else(|| "default".to_owned())
}
