use std::fs::File;
use std::io::{self, BufRead, BufReader, Lines, Read};

use anyhow::{Context, Result};

use crate::cli::Args;

/// Abstraction over input sources (stdin or a file).
///
/// Provides line-by-line iteration with a look-ahead buffer
/// for format detection.
pub struct InputStream {
    lines: Lines<BufReader<Box<dyn Read>>>,
    /// Buffered look-ahead lines consumed during detection.
    buffer: Vec<String>,
    /// Whether the buffer has been drained back to the caller.
    buffer_drained: bool,
}

impl InputStream {
    /// Open the input source described by `args`.
    pub fn new(args: &Args) -> Result<Self> {
        let reader: Box<dyn Read> = match &args.file {
            Some(path) => {
                let file =
                    File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
                Box::new(file)
            }
            None => Box::new(io::stdin()),
        };

        Ok(Self {
            lines: BufReader::new(reader).lines(),
            buffer: Vec::new(),
            buffer_drained: false,
        })
    }

    /// Read up to `n` lines into the look-ahead buffer for format detection.
    ///
    /// Returns a slice of the buffered lines. These lines will be yielded
    /// again when iterating with [`next_line`].
    pub fn peek(&mut self, n: usize) -> Result<&[String]> {
        while self.buffer.len() < n {
            match self.lines.next() {
                Some(Ok(line)) => self.buffer.push(line),
                Some(Err(e)) => return Err(e.into()),
                None => break,
            }
        }
        Ok(&self.buffer)
    }

    /// Return the next line of input.
    ///
    /// Drains the look-ahead buffer first, then reads from the underlying source.
    pub fn next_line(&mut self) -> Result<Option<String>> {
        // Drain buffered lines first.
        if !self.buffer_drained && !self.buffer.is_empty() {
            // Reverse so we can pop from the end efficiently.
            self.buffer.reverse();
            self.buffer_drained = true;
        }

        if self.buffer_drained {
            if let Some(line) = self.buffer.pop() {
                // Once fully drained, reset the flag.
                if self.buffer.is_empty() {
                    self.buffer_drained = false;
                }
                return Ok(Some(line));
            }
        }

        // Read from underlying source.
        match self.lines.next() {
            Some(Ok(line)) => Ok(Some(line)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }
}
