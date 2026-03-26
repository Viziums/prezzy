mod color;

pub use color::ColorDepth;

use crate::cli::{Args, ColorMode};

/// Captures everything we know about the user's terminal at startup.
#[derive(Debug)]
pub struct TerminalContext {
    /// Whether we should emit colors/styles.
    pub color_enabled: bool,

    /// Color depth the terminal supports.
    pub color_depth: ColorDepth,

    /// Terminal width in columns.
    pub width: u16,

    /// Whether stdout is a TTY.
    pub is_tty: bool,
}

impl TerminalContext {
    /// Probe the terminal and build a context from the environment + args.
    #[must_use] 
    pub fn detect(args: &Args) -> Self {
        let is_tty = crossterm::tty::IsTty::is_tty(&std::io::stdout());
        let color_depth = ColorDepth::detect();
        let width = args
            .width
            .unwrap_or_else(|| crossterm::terminal::size().map_or(80, |(w, _)| w));

        let color_enabled = resolve_color(args.color, is_tty, color_depth);

        Self {
            color_enabled,
            color_depth,
            width,
            is_tty,
        }
    }
}

/// Determine whether colors should be on, respecting the standard conventions:
///   1. `NO_COLOR` env var (see <https://no-color.org>) -- always wins.
///   2. `FORCE_COLOR` env var -- overrides TTY check.
///   3. `--color` flag.
///   4. TTY detection.
fn resolve_color(mode: ColorMode, is_tty: bool, depth: ColorDepth) -> bool {
    // NO_COLOR is the universal kill switch.
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }

    // FORCE_COLOR forces colors regardless of TTY.
    if std::env::var_os("FORCE_COLOR").is_some() {
        return !matches!(depth, ColorDepth::None);
    }

    match mode {
        ColorMode::Always => !matches!(depth, ColorDepth::None),
        ColorMode::Never => false,
        ColorMode::Auto => is_tty && !matches!(depth, ColorDepth::None),
    }
}
