use std::io::Write;
use std::process::{self, Command, Stdio};

use clap::Parser;

use prezzy::cli::Args;
use prezzy::config::Config;
use prezzy::theme;

fn main() {
    let mut args = Args::parse();

    // Handle meta commands that don't process input.
    if let Some(shell) = args.completions {
        Args::print_completions(shell);
        return;
    }

    if args.list_themes {
        for name in theme::THEME_NAMES {
            println!("{name}");
        }
        return;
    }

    // Load config and apply defaults.
    let config = Config::load();
    args.apply_config(&config);

    // Pager mode: re-run ourselves with stdout piped to `less -R`.
    if args.pager {
        run_with_pager(args);
        return;
    }

    if let Err(err) = prezzy::run(&args) {
        if is_broken_pipe(&err) {
            process::exit(0);
        }
        eprintln!("prezzy: {err:#}");
        process::exit(1);
    }
}

/// Spawn `less -R` and pipe our output through it.
fn run_with_pager(mut args: Args) {
    args.pager = false; // Prevent infinite recursion.

    // Build our own command line to re-exec.
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("prezzy: cannot find own executable: {e}");
            process::exit(1);
        }
    };

    let mut child_args: Vec<String> = Vec::new();
    child_args.push("--color=always".into()); // Force color for pager.
    child_args.push(format!("--theme={}", args.theme));
    if let Some(fmt) = args.format {
        child_args.push(format!("--format={fmt:?}").to_lowercase());
    }
    if let Some(ref level) = args.level {
        child_args.push(format!("--level={level}"));
    }
    if args.ascii {
        child_args.push("--ascii".into());
    }
    if let Some(ref file) = args.file {
        child_args.push(file.display().to_string());
    }

    let prezzy = Command::new(exe)
        .args(&child_args)
        .stdout(Stdio::piped())
        .stdin(Stdio::inherit())
        .spawn();

    let mut prezzy = match prezzy {
        Ok(c) => c,
        Err(e) => {
            eprintln!("prezzy: cannot spawn self: {e}");
            process::exit(1);
        }
    };

    let pager_name = if cfg!(windows) { "more" } else { "less" };
    let mut pager_cmd = Command::new(pager_name);
    if pager_name == "less" {
        pager_cmd.arg("-R"); // Pass through ANSI escapes.
    }
    pager_cmd.stdin(prezzy.stdout.take().unwrap());

    if let Ok(status) = pager_cmd.status() {
        let _ = prezzy.wait();
        process::exit(status.code().unwrap_or(0));
    } else {
        // Pager not found -- dump output directly.
        let output = prezzy.wait_with_output().unwrap_or_else(|e| {
            eprintln!("prezzy: {e}");
            process::exit(1);
        });
        let _ = std::io::stdout().write_all(&output.stdout);
    }
}

fn is_broken_pipe(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
            if io_err.kind() == std::io::ErrorKind::BrokenPipe {
                return true;
            }
        }
    }
    false
}
