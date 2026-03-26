use crossterm::style::Color;

use crate::cli::Args;

/// Color palette for JSON tokens.
#[derive(Debug, Clone)]
pub struct JsonColors {
    pub key: Color,
    pub string: Color,
    pub number: Color,
    pub bool_val: Color,
    pub null: Color,
    pub bracket: Color,
}

/// Color palette for log levels.
#[derive(Debug, Clone)]
pub struct LogColors {
    pub error: Color,
    pub warn: Color,
    pub info: Color,
    pub debug: Color,
    pub trace: Color,
    pub timestamp: Color,
    pub context: Color,
}

/// Color palette for diffs.
#[derive(Debug, Clone)]
pub struct DiffColors {
    pub add: Color,
    pub remove: Color,
    pub header: Color,
    pub context: Color,
}

/// A color theme controlling how prezzy renders output.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub json: JsonColors,
    pub log: LogColors,
    pub diff: DiffColors,

    // General
    pub url: Color,
    pub dim: Color,
    pub plain: Color,
}

impl Theme {
    /// Load the theme specified by CLI args.
    #[must_use]
    pub fn from_args(_args: &Args) -> Self {
        // Future: match on args.theme to load named themes.
        Self::default_theme()
    }

    /// The built-in default theme. Designed for dark backgrounds
    /// with reasonable legibility on light backgrounds.
    fn default_theme() -> Self {
        Self {
            name: "default".into(),

            json: JsonColors {
                key: Color::Cyan,
                string: Color::Green,
                number: Color::Yellow,
                bool_val: Color::Magenta,
                null: Color::DarkGrey,
                bracket: Color::White,
            },

            log: LogColors {
                error: Color::Red,
                warn: Color::Yellow,
                info: Color::Green,
                debug: Color::DarkGrey,
                trace: Color::DarkGrey,
                timestamp: Color::DarkGrey,
                context: Color::Blue,
            },

            diff: DiffColors {
                add: Color::Green,
                remove: Color::Red,
                header: Color::Cyan,
                context: Color::DarkGrey,
            },

            url: Color::Blue,
            dim: Color::DarkGrey,
            plain: Color::Reset,
        }
    }
}
