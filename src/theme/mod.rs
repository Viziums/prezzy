use crossterm::style::Color;

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
    pub url: Color,
    pub dim: Color,
    pub plain: Color,
}

/// All available built-in theme names.
pub const THEME_NAMES: &[&str] = &[
    "default",
    "monokai",
    "dracula",
    "solarized-dark",
    "solarized-light",
    "nord",
    "gruvbox",
];

impl Theme {
    /// Load a theme by name. Falls back to default for unknown names.
    #[must_use]
    pub fn by_name(name: &str) -> Self {
        match name {
            "monokai" => Self::monokai(),
            "dracula" => Self::dracula(),
            "solarized-dark" => Self::solarized_dark(),
            "solarized-light" => Self::solarized_light(),
            "nord" => Self::nord(),
            "gruvbox" => Self::gruvbox(),
            _ => Self::default_theme(),
        }
    }

    // ─── Default ────────────────────────────────────────────────

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

    // ─── Monokai ────────────────────────────────────────────────

    fn monokai() -> Self {
        Self {
            name: "monokai".into(),
            json: JsonColors {
                key: Color::AnsiValue(81),     // light blue
                string: Color::AnsiValue(186), // yellow-green
                number: Color::AnsiValue(141), // purple
                bool_val: Color::AnsiValue(141),
                null: Color::AnsiValue(242),
                bracket: Color::AnsiValue(252),
            },
            log: LogColors {
                error: Color::AnsiValue(197), // pink-red
                warn: Color::AnsiValue(208),  // orange
                info: Color::AnsiValue(148),  // green
                debug: Color::AnsiValue(242),
                trace: Color::AnsiValue(239),
                timestamp: Color::AnsiValue(242),
                context: Color::AnsiValue(81),
            },
            diff: DiffColors {
                add: Color::AnsiValue(148),
                remove: Color::AnsiValue(197),
                header: Color::AnsiValue(81),
                context: Color::AnsiValue(242),
            },
            url: Color::AnsiValue(81),
            dim: Color::AnsiValue(242),
            plain: Color::Reset,
        }
    }

    // ─── Dracula ────────────────────────────────────────────────

    fn dracula() -> Self {
        Self {
            name: "dracula".into(),
            json: JsonColors {
                key: Color::AnsiValue(117),    // cyan
                string: Color::AnsiValue(228), // yellow
                number: Color::AnsiValue(183), // purple
                bool_val: Color::AnsiValue(183),
                null: Color::AnsiValue(246),
                bracket: Color::AnsiValue(255),
            },
            log: LogColors {
                error: Color::AnsiValue(210), // red
                warn: Color::AnsiValue(228),  // yellow
                info: Color::AnsiValue(84),   // green
                debug: Color::AnsiValue(246),
                trace: Color::AnsiValue(242),
                timestamp: Color::AnsiValue(246),
                context: Color::AnsiValue(117),
            },
            diff: DiffColors {
                add: Color::AnsiValue(84),
                remove: Color::AnsiValue(210),
                header: Color::AnsiValue(183),
                context: Color::AnsiValue(246),
            },
            url: Color::AnsiValue(117),
            dim: Color::AnsiValue(246),
            plain: Color::Reset,
        }
    }

    // ─── Solarized Dark ─────────────────────────────────────────

    fn solarized_dark() -> Self {
        Self {
            name: "solarized-dark".into(),
            json: JsonColors {
                key: Color::AnsiValue(37),       // cyan
                string: Color::AnsiValue(64),    // green
                number: Color::AnsiValue(136),   // yellow
                bool_val: Color::AnsiValue(166), // orange
                null: Color::AnsiValue(246),
                bracket: Color::AnsiValue(252),
            },
            log: LogColors {
                error: Color::AnsiValue(160),
                warn: Color::AnsiValue(136),
                info: Color::AnsiValue(64),
                debug: Color::AnsiValue(246),
                trace: Color::AnsiValue(242),
                timestamp: Color::AnsiValue(242),
                context: Color::AnsiValue(33),
            },
            diff: DiffColors {
                add: Color::AnsiValue(64),
                remove: Color::AnsiValue(160),
                header: Color::AnsiValue(37),
                context: Color::AnsiValue(242),
            },
            url: Color::AnsiValue(33),
            dim: Color::AnsiValue(242),
            plain: Color::Reset,
        }
    }

    // ─── Solarized Light ────────────────────────────────────────

    fn solarized_light() -> Self {
        Self {
            name: "solarized-light".into(),
            json: JsonColors {
                key: Color::AnsiValue(37),
                string: Color::AnsiValue(64),
                number: Color::AnsiValue(136),
                bool_val: Color::AnsiValue(166),
                null: Color::AnsiValue(240),
                bracket: Color::AnsiValue(235),
            },
            log: LogColors {
                error: Color::AnsiValue(160),
                warn: Color::AnsiValue(136),
                info: Color::AnsiValue(64),
                debug: Color::AnsiValue(246),
                trace: Color::AnsiValue(248),
                timestamp: Color::AnsiValue(246),
                context: Color::AnsiValue(33),
            },
            diff: DiffColors {
                add: Color::AnsiValue(64),
                remove: Color::AnsiValue(160),
                header: Color::AnsiValue(37),
                context: Color::AnsiValue(246),
            },
            url: Color::AnsiValue(33),
            dim: Color::AnsiValue(246),
            plain: Color::Reset,
        }
    }

    // ─── Nord ───────────────────────────────────────────────────

    fn nord() -> Self {
        Self {
            name: "nord".into(),
            json: JsonColors {
                key: Color::AnsiValue(110),      // frost blue
                string: Color::AnsiValue(108),   // green
                number: Color::AnsiValue(179),   // yellow
                bool_val: Color::AnsiValue(139), // purple
                null: Color::AnsiValue(243),
                bracket: Color::AnsiValue(252),
            },
            log: LogColors {
                error: Color::AnsiValue(174), // red
                warn: Color::AnsiValue(179),
                info: Color::AnsiValue(108),
                debug: Color::AnsiValue(243),
                trace: Color::AnsiValue(240),
                timestamp: Color::AnsiValue(243),
                context: Color::AnsiValue(110),
            },
            diff: DiffColors {
                add: Color::AnsiValue(108),
                remove: Color::AnsiValue(174),
                header: Color::AnsiValue(110),
                context: Color::AnsiValue(243),
            },
            url: Color::AnsiValue(110),
            dim: Color::AnsiValue(243),
            plain: Color::Reset,
        }
    }

    // ─── Gruvbox ────────────────────────────────────────────────

    fn gruvbox() -> Self {
        Self {
            name: "gruvbox".into(),
            json: JsonColors {
                key: Color::AnsiValue(109),      // blue
                string: Color::AnsiValue(142),   // green
                number: Color::AnsiValue(214),   // yellow
                bool_val: Color::AnsiValue(175), // purple
                null: Color::AnsiValue(245),
                bracket: Color::AnsiValue(223),
            },
            log: LogColors {
                error: Color::AnsiValue(167), // red
                warn: Color::AnsiValue(214),
                info: Color::AnsiValue(142),
                debug: Color::AnsiValue(245),
                trace: Color::AnsiValue(241),
                timestamp: Color::AnsiValue(245),
                context: Color::AnsiValue(109),
            },
            diff: DiffColors {
                add: Color::AnsiValue(142),
                remove: Color::AnsiValue(167),
                header: Color::AnsiValue(109),
                context: Color::AnsiValue(245),
            },
            url: Color::AnsiValue(109),
            dim: Color::AnsiValue(245),
            plain: Color::Reset,
        }
    }
}
