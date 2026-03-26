use std::path::PathBuf;

use clap::{Parser, ValueEnum};

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

    /// Color theme to use.
    #[arg(short, long, default_value = "default")]
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
