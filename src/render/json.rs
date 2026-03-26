use std::io::Write;

use anyhow::Result;
use crossterm::style::{Color, Stylize};

use super::{RenderContext, Renderer};

/// Renders JSON with syntax highlighting and pretty indentation.
pub struct JsonRenderer;

impl Renderer for JsonRenderer {
    fn wants_full_input(&self) -> bool {
        true
    }

    fn render_line(&self, line: &str, writer: &mut dyn Write, _ctx: &RenderContext) -> Result<()> {
        write!(writer, "{line}")?;
        Ok(())
    }

    fn render_all(&self, input: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        let trimmed = input.trim();

        let value: serde_json::Value = if let Ok(v) = serde_json::from_str(trimmed) {
            v
        } else {
            write!(writer, "{input}")?;
            return Ok(());
        };

        let pretty = serde_json::to_string_pretty(&value)?;

        if ctx.terminal.color_enabled {
            write_highlighted_json(&pretty, writer, &ctx.theme.json)?;
        } else {
            write!(writer, "{pretty}")?;
        }

        Ok(())
    }
}

/// Highlight a single JSON line (used by both JSON and NDJSON renderers).
pub fn write_highlighted_json(json: &str, writer: &mut dyn Write, colors: &super::super::theme::JsonColors) -> Result<()> {
    let tokens = tokenize_json(json);
    for token in &tokens {
        let color = match token.kind {
            TokenKind::Key => colors.key,
            TokenKind::StringVal => colors.string,
            TokenKind::Number => colors.number,
            TokenKind::Bool => colors.bool_val,
            TokenKind::Null => colors.null,
            TokenKind::Bracket => colors.bracket,
            TokenKind::Punctuation | TokenKind::Whitespace => Color::Reset,
        };

        if matches!(token.kind, TokenKind::Punctuation | TokenKind::Whitespace) {
            write!(writer, "{}", token.text)?;
        } else {
            write!(writer, "{}", token.text.with(color))?;
        }
    }
    Ok(())
}

// ─── Tokenizer ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Key,
    StringVal,
    Number,
    Bool,
    Null,
    Bracket,
    Punctuation,
    Whitespace,
}

struct Token<'a> {
    kind: TokenKind,
    text: &'a str,
}

/// Fast single-pass tokenizer for pretty-printed JSON.
/// Operates on string slices (zero-allocation for token text).
///
/// Tracks an object/array context stack to correctly distinguish
/// keys (in objects) from string values (in arrays or after colons).
fn tokenize_json(json: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let bytes = json.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut after_colon = false;
    // Stack: true = inside object, false = inside array.
    let mut ctx_stack: Vec<bool> = Vec::new();

    let in_array = |stack: &[bool]| -> bool {
        stack.last().copied() == Some(false)
    };

    while i < len {
        let start = i;
        match bytes[i] {
            b' ' | b'\t' | b'\n' | b'\r' => {
                while i < len && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
                    i += 1;
                }
                tokens.push(Token { kind: TokenKind::Whitespace, text: &json[start..i] });
            }
            b'"' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i] == b'"' {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                // A string is a value if: (a) we're after ':', or (b) we're in an array.
                let is_value = after_colon || in_array(&ctx_stack);
                let kind = if is_value { TokenKind::StringVal } else { TokenKind::Key };
                tokens.push(Token { kind, text: &json[start..i] });
                after_colon = false;
            }
            b':' => {
                i += 1;
                after_colon = true;
                tokens.push(Token { kind: TokenKind::Punctuation, text: &json[start..i] });
            }
            b',' => {
                i += 1;
                after_colon = false;
                tokens.push(Token { kind: TokenKind::Punctuation, text: &json[start..i] });
            }
            b'{' => {
                i += 1;
                ctx_stack.push(true); // object
                after_colon = false;
                tokens.push(Token { kind: TokenKind::Bracket, text: &json[start..i] });
            }
            b'[' => {
                i += 1;
                ctx_stack.push(false); // array
                after_colon = false;
                tokens.push(Token { kind: TokenKind::Bracket, text: &json[start..i] });
            }
            b'}' | b']' => {
                i += 1;
                ctx_stack.pop();
                after_colon = false;
                tokens.push(Token { kind: TokenKind::Bracket, text: &json[start..i] });
            }
            b't' if json[i..].starts_with("true") => {
                i += 4;
                after_colon = false;
                tokens.push(Token { kind: TokenKind::Bool, text: &json[start..i] });
            }
            b'f' if json[i..].starts_with("false") => {
                i += 5;
                after_colon = false;
                tokens.push(Token { kind: TokenKind::Bool, text: &json[start..i] });
            }
            b'n' if json[i..].starts_with("null") => {
                i += 4;
                after_colon = false;
                tokens.push(Token { kind: TokenKind::Null, text: &json[start..i] });
            }
            b'0'..=b'9' | b'-' => {
                while i < len && matches!(bytes[i], b'0'..=b'9' | b'.' | b'-' | b'+' | b'e' | b'E') {
                    i += 1;
                }
                after_colon = false;
                tokens.push(Token { kind: TokenKind::Number, text: &json[start..i] });
            }
            _ => {
                i += 1;
                tokens.push(Token { kind: TokenKind::Punctuation, text: &json[start..i] });
            }
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_identifies_keys_vs_values() {
        let json = r#"{"name": "prezzy", "count": 42, "ok": true, "x": null}"#;
        let tokens = tokenize_json(json);

        let keys: Vec<&str> = tokens.iter()
            .filter(|t| t.kind == TokenKind::Key)
            .map(|t| t.text)
            .collect();
        assert_eq!(keys, vec![r#""name""#, r#""count""#, r#""ok""#, r#""x""#]);

        let strings: Vec<&str> = tokens.iter()
            .filter(|t| t.kind == TokenKind::StringVal)
            .map(|t| t.text)
            .collect();
        assert_eq!(strings, vec![r#""prezzy""#]);

        assert!(tokens.iter().any(|t| t.kind == TokenKind::Number && t.text == "42"));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Bool && t.text == "true"));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Null && t.text == "null"));
    }

    #[test]
    fn tokenizer_handles_arrays() {
        let json = r#"["a", "b", 1]"#;
        let tokens = tokenize_json(json);

        // In arrays, strings are values not keys
        let strings: Vec<&str> = tokens.iter()
            .filter(|t| t.kind == TokenKind::StringVal)
            .map(|t| t.text)
            .collect();
        assert_eq!(strings, vec![r#""a""#, r#""b""#]);
    }
}
