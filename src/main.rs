use std::io::Write;
use std::process::{self, Command, Stdio};

use clap::Parser;

use prezzy::cli::Args;
use prezzy::config::Config;
use prezzy::theme;

fn main() {
    let mut args = Args::parse();

    // Subcommands — run before pipe-mode logic.
    match args.command {
        Some(prezzy::cli::Command::Shell(ref shell_args)) => {
            if let Err(err) = prezzy::shell::run(shell_args) {
                eprintln!("prezzy: {err:#}");
                process::exit(1);
            }
            return;
        }
        Some(prezzy::cli::Command::History(ref history_args)) => {
            if let Err(err) = run_history(history_args) {
                eprintln!("prezzy: {err:#}");
                process::exit(1);
            }
            return;
        }
        None => {}
    }

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

fn run_history(args: &prezzy::cli::HistoryArgs) -> anyhow::Result<()> {
    use prezzy::history::{self, HistoryDb};

    let path = history::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("cannot determine data directory"))?;

    if !path.exists() && !args.clear {
        eprintln!("prezzy: no history yet — run `prezzy shell` first.");
        return Ok(());
    }

    let db = HistoryDb::open(&path)?;

    if args.clear {
        let count = db.clear()?;
        println!("Deleted {count} entries.");
        return Ok(());
    }

    if args.stats {
        let s = db.stats()?;
        println!("Total commands:  {}", s.total_commands);
        println!("Unique commands: {}", s.unique_commands);
        println!("Failed commands: {}", s.failed_commands);
        if let Some(avg) = s.avg_duration_ms {
            println!("Avg duration:    {avg:.0}ms");
        }
        if s.total_commands > 0 {
            #[allow(clippy::cast_precision_loss)]
            let rate =
                (s.total_commands - s.failed_commands) as f64 / s.total_commands as f64 * 100.0;
            println!("Success rate:    {rate:.1}%");
        }
        return Ok(());
    }

    if let Some(n) = args.top {
        let top = db.top(n)?;
        if top.is_empty() {
            println!("No history.");
            return Ok(());
        }
        // Right-align the count column.
        let max_count = top.iter().map(|(_, c)| *c).max().unwrap_or(0);
        let count_width = format!("{max_count}").len();
        for (cmd, count) in &top {
            println!("{count:>count_width$}  {cmd}");
        }
        return Ok(());
    }

    let records = if args.failed {
        db.failed(args.limit)?
    } else if args.slow {
        db.slowest(args.limit)?
    } else if let Some(ref pattern) = args.search {
        db.search(pattern, args.limit)?
    } else if args.today {
        let day_ago = history::now_ms() - 86_400_000;
        db.since(day_ago, args.limit)?
    } else if args.week {
        let week_ago = history::now_ms() - 7 * 86_400_000;
        db.since(week_ago, args.limit)?
    } else if let Some(ref dir) = args.dir {
        db.by_dir(dir, args.limit)?
    } else {
        db.recent(args.limit)?
    };

    if records.is_empty() {
        println!("No matching commands.");
        return Ok(());
    }

    if args.export {
        println!("command,timestamp,duration_ms,exit_code,cwd,format");
        for r in &records {
            println!(
                "\"{}\",{},{},{},{},{}",
                r.command.replace('"', "\"\""),
                r.timestamp_ms,
                r.duration_ms.map_or(String::new(), |d| d.to_string()),
                r.exit_code.map_or(String::new(), |c| c.to_string()),
                r.cwd.as_deref().unwrap_or(""),
                r.format.as_deref().unwrap_or(""),
            );
        }
        return Ok(());
    }

    for record in &records {
        print_record(record);
    }

    Ok(())
}

fn print_record(r: &prezzy::history::CommandRecord) {
    use std::fmt::Write;

    // Format timestamp as human-readable.
    let secs = r.timestamp_ms / 1000;
    let dt = chrono_lite(secs);

    let mut meta = String::new();
    if let Some(code) = r.exit_code {
        if code != 0 {
            write!(meta, " [exit {code}]").unwrap();
        }
    }
    if let Some(ms) = r.duration_ms {
        if ms >= 1000 {
            #[allow(clippy::cast_precision_loss)]
            write!(meta, " ({:.1}s)", ms as f64 / 1000.0).unwrap();
        } else {
            write!(meta, " ({ms}ms)").unwrap();
        }
    }
    if let Some(ref cwd) = r.cwd {
        write!(meta, " {cwd}").unwrap();
    }

    println!("{dt}  {}{meta}", r.command);
}

/// Minimal timestamp formatting without pulling in chrono.
fn chrono_lite(epoch_secs: i64) -> String {
    // Use platform time formatting. On failure, return raw epoch.
    #[cfg(unix)]
    {
        // strftime via libc is unsafe, just do simple math.
        let _ = epoch_secs;
    }

    // Simple approach: format as "YYYY-MM-DD HH:MM" using basic math.
    // This is UTC, which is fine for a CLI tool.
    const SECS_PER_DAY: i64 = 86400;
    const SECS_PER_HOUR: i64 = 3600;
    const SECS_PER_MIN: i64 = 60;

    let days = epoch_secs / SECS_PER_DAY;
    let time_of_day = epoch_secs % SECS_PER_DAY;
    let hours = time_of_day / SECS_PER_HOUR;
    let minutes = (time_of_day % SECS_PER_HOUR) / SECS_PER_MIN;

    // Days since epoch → year/month/day (simplified civil calendar).
    let (y, m, d) = days_to_ymd(days + 719_468); // shift to 0000-03-01 epoch
    format!("{y:04}-{m:02}-{d:02} {hours:02}:{minutes:02}")
}

/// Convert day count to (year, month, day). Algorithm from Howard Hinnant.
const fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
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
