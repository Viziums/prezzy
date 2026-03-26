use std::process;

use clap::Parser;

use prezzy::cli::Args;

fn main() {
    let args = Args::parse();

    if let Err(err) = prezzy::run(&args) {
        // If stdout is broken (e.g. `prezzy | head` closes the pipe), exit silently.
        if is_broken_pipe(&err) {
            process::exit(0);
        }
        eprintln!("prezzy: {err:#}");
        process::exit(1);
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
