use std::path::PathBuf;

use serde::Deserialize;

/// User configuration loaded from `~/.config/prezzy/config.toml`.
///
/// All fields are optional -- CLI flags override config values,
/// and config values override built-in defaults.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Default color theme name.
    pub theme: Option<String>,
    /// Default color mode (auto, always, never).
    pub color: Option<String>,
    /// Use ASCII box-drawing characters instead of Unicode.
    pub ascii: Option<bool>,
    /// Default format override.
    pub format: Option<String>,
    /// Default log level filter.
    pub level: Option<String>,
}

impl Config {
    /// Load config from the platform-appropriate path.
    ///
    /// Returns `Config::default()` if the file doesn't exist or is malformed
    /// (we never fail on config -- bad config should not break the tool).
    #[must_use]
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };

        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };

        match toml::from_str(&content) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("prezzy: warning: invalid config at {}: {e}", path.display());
                Self::default()
            }
        }
    }

    /// Platform-appropriate config file path.
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("prezzy").join("config.toml"))
    }
}
