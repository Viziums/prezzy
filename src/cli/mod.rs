use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
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
    #[arg(long, env = "PREZZY_ASCII", value_parser = parse_bool_env, num_args = 0, default_missing_value = "true")]
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

    /// Subcommand (e.g. `prezzy shell`).
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Launch an interactive shell with automatic output beautification.
    ///
    /// Wraps your shell in a PTY, detects command output formats, and renders
    /// them with syntax highlighting — transparently, without changing how
    /// your shell works. Programs using alternate screen (vim, htop, less)
    /// are passed through unmodified.
    Shell(ShellArgs),
}

/// Arguments for `prezzy shell`.
#[derive(Debug, clap::Args)]
pub struct ShellArgs {
    /// Color theme.
    #[arg(short, long, default_value = "default", env = "PREZZY_THEME")]
    pub theme: String,

    /// Filter log output by minimum level.
    #[arg(short, long)]
    pub level: Option<String>,

    /// Use ASCII characters instead of Unicode box-drawing.
    #[arg(long, env = "PREZZY_ASCII", value_parser = parse_bool_env, num_args = 0, default_missing_value = "true")]
    pub ascii: bool,

    /// Disable beautification (pure PTY passthrough).
    /// Useful for debugging or when beautification interferes with a program.
    #[arg(long)]
    pub passthrough: bool,
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

/// Parse boolean values from env vars, accepting 1/0/true/false/yes/no.
fn parse_bool_env(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" | "" => Ok(false),
        other => Err(format!("invalid boolean value '{other}', expected true/false/1/0/yes/no")),
    }
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
