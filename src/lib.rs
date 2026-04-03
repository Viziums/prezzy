pub mod cli;
pub mod config;
pub mod detect;
pub mod history;
pub mod input;
pub mod render;
pub mod shell;
pub mod terminal;
pub mod theme;

use anyhow::{Context, Result};
use cli::Args;
use input::InputStream;
use render::RenderEngine;
use terminal::TerminalContext;
use theme::Theme;

/// Core pipeline: read stdin -> detect format -> render beautifully -> write stdout.
pub fn run(args: &Args) -> Result<()> {
    let terminal = TerminalContext::detect(args);
    let theme = Theme::by_name(&args.theme);

    if args.watch {
        return run_watch(args, &terminal, &theme);
    }

    let mut input = InputStream::new(args)?;
    let mut engine = RenderEngine::new(&terminal, &theme, args);

    engine.process(&mut input)?;

    Ok(())
}

/// Watch mode: continuously read input (file or stdin) and beautify line-by-line.
///
/// Detects format from the first batch of lines, then renders every subsequent
/// line through that renderer, flushing after each line for real-time output.
/// When reading a file, polls for new content like `tail -f`.
fn run_watch(args: &Args, terminal: &TerminalContext, theme: &Theme) -> Result<()> {
    use std::io::{self, BufRead, BufReader, Write};

    let level_filter = args.level.as_deref().and_then(render::LevelFilter::parse);
    let ctx = render::RenderContext {
        terminal,
        theme,
        level_filter,
        ascii: args.ascii,
    };

    let is_file = args.file.is_some();
    let reader: Box<dyn io::Read> = match &args.file {
        Some(path) => {
            let file = std::fs::File::open(path)
                .with_context(|| format!("cannot open {}", path.display()))?;
            Box::new(file)
        }
        None => Box::new(io::stdin()),
    };

    let mut reader = BufReader::new(reader);
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut line_buf = String::new();

    // Read a single line, returning Ok(true) if a line was read, Ok(false) on EOF.
    let read_line = |reader: &mut BufReader<Box<dyn io::Read>>,
                          buf: &mut String|
     -> io::Result<bool> {
        buf.clear();
        match reader.read_line(buf) {
            Ok(0) => Ok(false),
            Ok(_) => {
                // Strip trailing newline.
                if buf.ends_with('\n') {
                    buf.pop();
                    if buf.ends_with('\r') {
                        buf.pop();
                    }
                }
                Ok(true)
            }
            Err(e) => Err(e),
        }
    };

    // Buffer initial lines for detection.
    let mut detection_lines: Vec<String> = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(200);

    while detection_lines.len() < detect::DETECTION_BUFFER_SIZE {
        if std::time::Instant::now() > deadline && !detection_lines.is_empty() {
            break;
        }
        match read_line(&mut reader, &mut line_buf) {
            Ok(true) => detection_lines.push(line_buf.clone()),
            Ok(false) => {
                if is_file {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    continue;
                }
                break;
            }
            Err(e) => return Err(e.into()),
        }
    }

    let format = detect::detect_format(&detection_lines, args.format);
    let renderer = render::renderer_for(format);

    // Render the buffered detection lines.
    for line in &detection_lines {
        renderer.render_line(line, &mut stdout, &ctx)?;
        writeln!(stdout)?;
        stdout.flush()?;
    }
    drop(detection_lines);

    // Stream remaining lines, flushing after each for real-time output.
    loop {
        match read_line(&mut reader, &mut line_buf) {
            Ok(true) => {
                renderer.render_line(&line_buf, &mut stdout, &ctx)?;
                writeln!(stdout)?;
                stdout.flush()?;
            }
            Ok(false) => {
                if is_file {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                break;
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::BrokenPipe {
                    break;
                }
                return Err(e.into());
            }
        }
    }

    Ok(())
}
