use std::io::Write;

use anyhow::Result;
use crossterm::style::{Attribute, Color, Stylize};
use regex::Regex;
use std::sync::LazyLock;

use super::{RenderContext, Renderer};

/// Renders stack traces with highlighted error messages, dimmed
/// library frames, and emphasized user code.
pub struct StackTraceRenderer;

impl Renderer for StackTraceRenderer {
    fn render_line(&self, line: &str, writer: &mut dyn Write, ctx: &RenderContext) -> Result<()> {
        if !ctx.terminal.color_enabled {
            write!(writer, "{line}")?;
            return Ok(());
        }

        let kind = classify_line(line);
        match kind {
            LineKind::ErrorMessage => {
                write!(
                    writer,
                    "{}",
                    line.with(Color::Red).attribute(Attribute::Bold)
                )?;
            }
            LineKind::UserFrame => {
                write_frame_highlighted(line, writer, Color::White)?;
            }
            LineKind::LibraryFrame | LineKind::FilePath => {
                write!(writer, "{}", line.with(Color::DarkGrey))?;
            }
            LineKind::FrameCode => {
                write!(writer, "{}", line.with(Color::White))?;
            }
            LineKind::Header => {
                write!(writer, "{}", line.with(Color::Yellow))?;
            }
            LineKind::Other => {
                write!(writer, "{line}")?;
            }
        }

        Ok(())
    }
}

// ─── Line classification ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    /// Exception/error message line (bold red).
    ErrorMessage,
    /// A stack frame in user code (bright).
    UserFrame,
    /// A stack frame in library/framework code (dim).
    LibraryFrame,
    /// A file path line (Go-style, below the function).
    FilePath,
    /// Source code shown inline in the trace (Python).
    FrameCode,
    /// Trace header (e.g., "Traceback ...", "goroutine ...").
    Header,
    /// Anything else.
    Other,
}

// ─── Patterns ───────────────────────────────────────────────────

static ERROR_MSG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(\w+\.)*\w*(Error|Exception|Panic)\b").unwrap());

static HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)^(?:",
        r"Traceback \(most recent call last\):",
        r"|thread '.*' panicked at",
        r"|goroutine \d+ \[",
        r"|stack backtrace:",
        r"|Caused by:",
        r")"
    ))
    .unwrap()
});

/// Python frame: `  File "/app/main.py", line 42, in handler`
static PY_FRAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(\s+File ")(.+)(", line )(\d+)(.*)"#).unwrap());

/// JS/Java/C# frame: `    at something(location)`
static AT_FRAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s+at\s+)(.+)$").unwrap());

/// Rust frame: `   N: module::function`
static RUST_FRAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s+\d+:\s+)(.+)$").unwrap());

/// Go file path line (tab-indented path after function).
static GO_FILE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\t.+\.\w+:\d+").unwrap());

/// Python inline source code.
static PY_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^    \S").unwrap());

/// Ruby frame: `from /path:N:in 'method'`.
static RUBY_FRAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+from\s+.+:\d+:in\s+").unwrap());

/// Heuristic: paths containing these are likely library code.
const LIBRARY_INDICATORS: &[&str] = &[
    "node_modules",
    "site-packages",
    "/usr/lib",
    "/usr/local/lib",
    ".cargo/registry",
    ".rustup/toolchains",
    "vendor/",
    "pkg/mod/",
    "GOROOT",
    "/go/pkg/",
    "java.base/",
    "jdk.internal",
    "org.springframework",
    "io.netty",
    "scala.runtime",
    "node:internal",
    "<frozen",
    "native methods",
    "Unknown Source",
    // Rust standard library and runtime
    "std::",
    "core::",
    "alloc::",
    "tokio::",
    "hyper::",
    "actix",
];

fn classify_line(line: &str) -> LineKind {
    if HEADER.is_match(line) {
        return LineKind::Header;
    }

    if ERROR_MSG.is_match(line) {
        return LineKind::ErrorMessage;
    }

    // Python frame.
    if PY_FRAME.is_match(line) {
        return if is_library_path(line) {
            LineKind::LibraryFrame
        } else {
            LineKind::UserFrame
        };
    }

    // JS/Java/C#/general `at` frame.
    if AT_FRAME.is_match(line) {
        return if is_library_path(line) {
            LineKind::LibraryFrame
        } else {
            LineKind::UserFrame
        };
    }

    // Rust frame.
    if RUST_FRAME.is_match(line) {
        return if is_library_path(line) {
            LineKind::LibraryFrame
        } else {
            LineKind::UserFrame
        };
    }

    // Ruby frame.
    if RUBY_FRAME.is_match(line) {
        return if is_library_path(line) {
            LineKind::LibraryFrame
        } else {
            LineKind::UserFrame
        };
    }

    // Go file path line.
    if GO_FILE.is_match(line) {
        return LineKind::FilePath;
    }

    // Python inline code (indented 4 spaces, then non-space).
    if PY_CODE.is_match(line) && !line.trim_start().starts_with("at ") {
        return LineKind::FrameCode;
    }

    LineKind::Other
}

fn is_library_path(line: &str) -> bool {
    let lower = line.to_lowercase();
    LIBRARY_INDICATORS
        .iter()
        .any(|ind| lower.contains(&ind.to_lowercase()))
}

/// Highlight a frame line: dim the leading whitespace and `at `
/// prefix, then show the frame body in the given color.
fn write_frame_highlighted(line: &str, writer: &mut dyn Write, color: Color) -> Result<()> {
    // Try to split at the frame body.
    if let Some(caps) = AT_FRAME.captures(line) {
        let prefix = caps.get(1).map_or("", |m| m.as_str());
        let body = caps.get(2).map_or("", |m| m.as_str());
        write!(
            writer,
            "{}{}",
            prefix.with(Color::DarkGrey),
            body.with(color)
        )?;
        return Ok(());
    }

    if let Some(caps) = RUST_FRAME.captures(line) {
        let prefix = caps.get(1).map_or("", |m| m.as_str());
        let body = caps.get(2).map_or("", |m| m.as_str());
        write!(
            writer,
            "{}{}",
            prefix.with(Color::DarkGrey),
            body.with(color)
        )?;
        return Ok(());
    }

    if let Some(caps) = PY_FRAME.captures(line) {
        let file_prefix = caps.get(1).map_or("", |m| m.as_str());
        let path = caps.get(2).map_or("", |m| m.as_str());
        let line_prefix = caps.get(3).map_or("", |m| m.as_str());
        let lineno = caps.get(4).map_or("", |m| m.as_str());
        let rest = caps.get(5).map_or("", |m| m.as_str());
        write!(
            writer,
            "{}{}{}{}{}",
            file_prefix.with(Color::DarkGrey),
            path.with(color),
            line_prefix.with(Color::DarkGrey),
            lineno.with(Color::Yellow),
            rest.with(color),
        )?;
        return Ok(());
    }

    // Fallback: color the whole line.
    write!(writer, "{}", line.with(color))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_python_traceback() {
        assert_eq!(
            classify_line("Traceback (most recent call last):"),
            LineKind::Header,
        );
        assert_eq!(
            classify_line(r#"  File "/app/main.py", line 42, in handler"#),
            LineKind::UserFrame,
        );
        assert_eq!(
            classify_line(r#"  File "/usr/lib/python3/json/decoder.py", line 355"#),
            LineKind::LibraryFrame,
        );
        assert_eq!(
            classify_line("ValueError: invalid input"),
            LineKind::ErrorMessage,
        );
    }

    #[test]
    fn classifies_js_stack() {
        assert_eq!(
            classify_line("TypeError: Cannot read property 'x' of undefined"),
            LineKind::ErrorMessage,
        );
        assert_eq!(
            classify_line("    at processItems (/app/src/handler.js:42:15)"),
            LineKind::UserFrame,
        );
        assert_eq!(
            classify_line("    at Module._compile (node:internal/modules/cjs/loader:1198:14)"),
            LineKind::LibraryFrame,
        );
    }

    #[test]
    fn classifies_java_stack() {
        assert_eq!(
            classify_line("java.lang.NullPointerException: null"),
            LineKind::ErrorMessage,
        );
        assert_eq!(
            classify_line("    at com.example.Service.process(Service.java:42)"),
            LineKind::UserFrame,
        );
        assert_eq!(
            classify_line(
                "    at org.springframework.web.servlet.DispatcherServlet.doDispatch(DispatcherServlet.java:1067)"
            ),
            LineKind::LibraryFrame,
        );
    }

    #[test]
    fn classifies_rust_stack() {
        assert_eq!(
            classify_line("thread 'main' panicked at 'oops', src/main.rs:42:10"),
            LineKind::Header,
        );
        assert_eq!(classify_line("   3: prezzy::main"), LineKind::UserFrame,);
        assert_eq!(
            classify_line("   0: std::panicking::begin_panic_handler"),
            LineKind::LibraryFrame,
        );
    }

    #[test]
    fn user_vs_library_detection() {
        assert!(!is_library_path("/app/src/handler.js:42:15"));
        assert!(is_library_path("node_modules/express/lib/router.js:44"));
        assert!(is_library_path("/usr/lib/python3/json/decoder.py"));
        assert!(is_library_path(".cargo/registry/src/serde-1.0/lib.rs"));
    }
}
