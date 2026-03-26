/// The color depth the terminal supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorDepth {
    /// No color support at all.
    None,
    /// Basic 4-bit (16 colors).
    Basic,
    /// 8-bit (256 colors).
    EightBit,
    /// 24-bit true color.
    TrueColor,
}

impl ColorDepth {
    /// Detect color depth from environment variables.
    ///
    /// Checks `COLORTERM`, then `TERM`, falling back to basic if a TTY.
    #[must_use]
    pub fn detect() -> Self {
        // COLORTERM is the most reliable signal for true color.
        if let Ok(ct) = std::env::var("COLORTERM") {
            let ct = ct.to_lowercase();
            if ct == "truecolor" || ct == "24bit" {
                return Self::TrueColor;
            }
        }

        // Check TERM for known capabilities.
        if let Ok(term) = std::env::var("TERM") {
            let term = term.to_lowercase();
            if term.contains("256color") {
                return Self::EightBit;
            }
            if term == "dumb" {
                return Self::None;
            }
        }

        // Windows Terminal and modern terminals generally support true color.
        if std::env::var_os("WT_SESSION").is_some() {
            return Self::TrueColor;
        }

        // Default: assume basic color if stdout is a TTY.
        if crossterm::tty::IsTty::is_tty(&std::io::stdout()) {
            Self::Basic
        } else {
            Self::None
        }
    }
}

impl std::fmt::Display for ColorDepth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Basic => write!(f, "basic (16)"),
            Self::EightBit => write!(f, "256"),
            Self::TrueColor => write!(f, "truecolor (24-bit)"),
        }
    }
}
