use std::io::Write;

use anyhow::Result;
use crossterm::style::{Attribute, Color, Stylize};
use unicode_width::UnicodeWidthStr;

use super::{RenderContext, Renderer};

/// Renders CSV/TSV as an aligned table with borders and a highlighted header row.
pub struct CsvRenderer {
    delimiter: char,
}

impl CsvRenderer {
    #[must_use]
    pub const fn comma() -> Self {
        Self { delimiter: ',' }
    }

    #[must_use]
    pub const fn tab() -> Self {
        Self { delimiter: '\t' }
    }
}

impl Renderer for CsvRenderer {
    fn wants_full_input(&self) -> bool {
        true
    }

    fn render_line(&self, line: &str, writer: &mut dyn Write, _ctx: &RenderContext) -> Result<()> {
        write!(writer, "{line}")?;
        Ok(())
    }

    fn render_all(&self, input: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        let rows = parse_rows(input, self.delimiter);
        if rows.is_empty() {
            return Ok(());
        }

        // Calculate column widths.
        let num_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut col_widths = vec![0usize; num_cols];
        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                col_widths[i] = col_widths[i].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }

        // Cap column widths to terminal width / num_cols (sensible max).
        let max_col =
            (ctx.terminal.width as usize).saturating_sub(num_cols * 3 + 1) / num_cols.max(1);
        for w in &mut col_widths {
            *w = (*w).min(max_col).max(1);
        }

        let colored = ctx.terminal.color_enabled;
        let border_color = Color::DarkGrey;
        let b = if ctx.ascii {
            &ASCII_BORDERS
        } else {
            &UNICODE_BORDERS
        };

        // Top border.
        write_border(
            writer,
            &col_widths,
            [b.top_left, b.top_mid, b.top_right, b.horiz],
            colored,
            border_color,
        )?;
        writeln!(writer)?;

        for (row_idx, row) in rows.iter().enumerate() {
            let pipe = b.vert;
            if colored {
                write!(writer, "{}", pipe.with(border_color))?;
            } else {
                write!(writer, "{pipe}")?;
            }

            for (col, width) in col_widths.iter().enumerate() {
                let cell = row.get(col).map_or("", String::as_str);
                let truncated = truncate_to_width(cell, *width);
                let padding = width.saturating_sub(UnicodeWidthStr::width(truncated.as_str()));

                write!(writer, " ")?;
                if colored && row_idx == 0 {
                    write!(
                        writer,
                        "{}",
                        truncated
                            .as_str()
                            .with(Color::Cyan)
                            .attribute(Attribute::Bold)
                    )?;
                } else {
                    write!(writer, "{truncated}")?;
                }
                write!(writer, "{:padding$} ", "")?;

                if colored {
                    write!(writer, "{}", pipe.with(border_color))?;
                } else {
                    write!(writer, "{pipe}")?;
                }
            }
            writeln!(writer)?;

            if row_idx == 0 {
                write_border(
                    writer,
                    &col_widths,
                    [b.mid_left, b.mid_mid, b.mid_right, b.horiz],
                    colored,
                    border_color,
                )?;
                writeln!(writer)?;
            }
        }

        write_border(
            writer,
            &col_widths,
            [b.bot_left, b.bot_mid, b.bot_right, b.horiz],
            colored,
            border_color,
        )?;

        Ok(())
    }
}

struct BorderChars {
    top_left: char,
    top_mid: char,
    top_right: char,
    mid_left: char,
    mid_mid: char,
    mid_right: char,
    bot_left: char,
    bot_mid: char,
    bot_right: char,
    horiz: char,
    vert: char,
}

const UNICODE_BORDERS: BorderChars = BorderChars {
    top_left: '┌',
    top_mid: '┬',
    top_right: '┐',
    mid_left: '├',
    mid_mid: '┼',
    mid_right: '┤',
    bot_left: '└',
    bot_mid: '┴',
    bot_right: '┘',
    horiz: '─',
    vert: '│',
};

const ASCII_BORDERS: BorderChars = BorderChars {
    top_left: '+',
    top_mid: '+',
    top_right: '+',
    mid_left: '+',
    mid_mid: '+',
    mid_right: '+',
    bot_left: '+',
    bot_mid: '+',
    bot_right: '+',
    horiz: '-',
    vert: '|',
};

/// `chars`: [left, mid, right, horiz]
fn write_border(
    writer: &mut dyn Write,
    widths: &[usize],
    chars: [char; 4],
    colored: bool,
    color: Color,
) -> Result<()> {
    let [left, mid, right, horiz] = chars;
    let mut s = String::with_capacity(widths.len() * 8);
    s.push(left);
    for (i, &w) in widths.iter().enumerate() {
        for _ in 0..w + 2 {
            s.push(horiz);
        }
        if i < widths.len() - 1 {
            s.push(mid);
        }
    }
    s.push(right);

    if colored {
        write!(writer, "{}", s.with(color))?;
    } else {
        write!(writer, "{s}")?;
    }
    Ok(())
}

/// Parse CSV input into rows of cells, handling quoted fields.
fn parse_rows(input: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        rows.push(parse_csv_line(line, delimiter));
    }
    rows
}

fn parse_csv_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    // Escaped quote.
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == delimiter {
            fields.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

/// Truncate a string to fit within `max_width` display columns.
fn truncate_to_width(s: &str, max_width: usize) -> String {
    let display_width = UnicodeWidthStr::width(s);
    if display_width <= max_width {
        return s.to_string();
    }

    // Reserve 1 column for the ellipsis.
    let target = max_width.saturating_sub(1);
    let mut width = 0;
    let mut result = String::new();
    for ch in s.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + w > target {
            break;
        }
        width += w;
        result.push(ch);
    }
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_csv() {
        let fields = parse_csv_line("Alice,30,NYC", ',');
        assert_eq!(fields, vec!["Alice", "30", "NYC"]);
    }

    #[test]
    fn parses_quoted_csv() {
        let fields = parse_csv_line(r#"Alice,"New York, NY",30"#, ',');
        assert_eq!(fields, vec!["Alice", "New York, NY", "30"]);
    }

    #[test]
    fn parses_tsv() {
        let fields = parse_csv_line("Alice\t30\tNYC", '\t');
        assert_eq!(fields, vec!["Alice", "30", "NYC"]);
    }

    #[test]
    fn truncates_long_strings() {
        assert_eq!(truncate_to_width("hello world", 5), "hell…");
        assert_eq!(truncate_to_width("hi", 5), "hi");
    }
}
