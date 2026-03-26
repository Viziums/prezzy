pub mod cli;
pub mod detect;
pub mod input;
pub mod render;
pub mod terminal;
pub mod theme;

use anyhow::Result;
use cli::Args;
use input::InputStream;
use render::RenderEngine;
use terminal::TerminalContext;
use theme::Theme;

/// Core pipeline: read stdin -> detect format -> render beautifully -> write stdout.
///
/// This is the main entry point for pipe mode (`cmd | prezzy`).
pub fn run(args: &Args) -> Result<()> {
    let terminal = TerminalContext::detect(args);
    let theme = Theme::from_args(args);
    let mut input = InputStream::new(args)?;
    let mut engine = RenderEngine::new(&terminal, &theme, args);

    engine.process(&mut input)?;

    Ok(())
}
