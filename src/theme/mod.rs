use crossterm::style::Color;

use crate::cli::Args;

/// A color theme controlling how prezzy renders output.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,

    // JSON / structured data
    pub json_key: Color,
    pub json_string: Color,
    pub json_number: Color,
    pub json_bool: Color,
    pub json_null: Color,
    pub json_bracket: Color,

    // Log levels
    pub log_error: Color,
    pub log_warn: Color,
    pub log_info: Color,
    pub log_debug: Color,
    pub log_trace: Color,
    pub log_timestamp: Color,

    // Diff
    pub diff_add: Color,
    pub diff_remove: Color,
    pub diff_header: Color,
    pub diff_context: Color,

    // General
    pub url: Color,
    pub keyword: Color,
    pub comment: Color,
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

            json_key: Color::Cyan,
            json_string: Color::Green,
            json_number: Color::Yellow,
            json_bool: Color::Magenta,
            json_null: Color::DarkGrey,
            json_bracket: Color::White,

            log_error: Color::Red,
            log_warn: Color::Yellow,
            log_info: Color::Green,
            log_debug: Color::DarkGrey,
            log_trace: Color::DarkGrey,
            log_timestamp: Color::DarkGrey,

            diff_add: Color::Green,
            diff_remove: Color::Red,
            diff_header: Color::Cyan,
            diff_context: Color::DarkGrey,

            url: Color::Blue,
            keyword: Color::Magenta,
            comment: Color::DarkGrey,
            dim: Color::DarkGrey,
            plain: Color::Reset,
        }
    }
}
