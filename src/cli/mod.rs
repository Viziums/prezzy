use std::path::PathBuf;

use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(
    name = "prezzy",
    about = "Make any CLI output beautiful. Zero config. Just pipe.",
    long_about = "Prezzy auto-detects the format of piped CLI output and applies \
                  syntax highlighting, structural formatting, and color.\n\n\
                  Usage:\n  \
                  command | prezzy\n  \
                  prezzy < file.json\n  \
                  prezzy file.json",
    version,
    after_help = "Examples:\n  \
                  curl -s https://api.github.com/users/octocat | prezzy\n  \
                  docker compose logs | prezzy\n  \
                  git diff | prezzy\n  \
                  cat data.csv | prezzy\n  \
                  env | prezzy"
)]
pub struct Args {
    /// File to read instead of stdin.
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Force a specific format instead of auto-detecting.
    #[arg(short, long, value_enum)]
    pub format: Option<FormatOverride>,

    /// Color theme (default, monokai, dracula, solarized-dark, solarized-light, nord, gruvbox).
    #[arg(short, long, default_value = "default", env = "PREZZY_THEME")]
    pub theme: String,

    /// Control when to use colors.
    #[arg(long, value_enum, default_value = "auto", env = "PREZZY_COLOR")]
    pub color: ColorMode,

    /// Override terminal width (columns).
    #[arg(short, long, env = "PREZZY_WIDTH")]
    pub width: Option<u16>,

    /// Filter log output by minimum level (trace, debug, info, warn, error).
    #[arg(short, long)]
    pub level: Option<String>,

    /// Use ASCII characters instead of Unicode box-drawing.
    #[arg(long, env = "PREZZY_ASCII")]
    pub ascii: bool,

    /// Pipe output through a pager (less).
    #[arg(long)]
    pub pager: bool,

    /// List available themes and exit.
    #[arg(long)]
    pub list_themes: bool,

    /// Generate shell completions and exit.
    #[arg(long, value_enum, hide = true)]
    pub completions: Option<Shell>,
}

impl Args {
    /// Apply config file defaults to any unset CLI args.
    pub fn apply_config(&mut self, config: &crate::config::Config) {
        if self.theme == "default" {
            if let Some(ref t) = config.theme {
                self.theme.clone_from(t);
            }
        }
        if self.level.is_none() {
            self.level.clone_from(&config.level);
        }
        if !self.ascii {
            self.ascii = config.ascii.unwrap_or(false);
        }
    }

    /// Print shell completions to stdout and exit.
    pub fn print_completions(shell: Shell) {
        let mut cmd = Self::command();
        clap_complete::generate(shell, &mut cmd, "prezzy", &mut std::io::stdout());
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ColorMode {
    /// Color when stdout is a terminal.
    Auto,
    /// Always emit colors.
    Always,
    /// Never emit colors.
    Never,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FormatOverride {
    Json,
    Ndjson,
    Yaml,
    Xml,
    Csv,
    Tsv,
    Log,
    Diff,
    Markdown,
    #[value(name = "kv")]
    KeyValue,
    Table,
    Plain,
}
