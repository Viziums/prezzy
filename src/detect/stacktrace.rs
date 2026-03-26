use regex::Regex;
use std::sync::LazyLock;

use super::{Detector, Format};

/// Detects stack traces from multiple programming languages.
///
/// Supported patterns:
///   - Python: `Traceback (most recent call last):` / `File "...", line N`
///   - JavaScript/TypeScript: `    at Function (file:line:col)` / `Error: msg`
///   - Java/Kotlin: `    at com.pkg.Class.method(File.java:N)`
///   - Rust: `thread '...' panicked at` / `   N: module::function`
///   - Go: `goroutine N [running]:` / `pkg.func(file.go:N)`
///   - C#/.NET: `   at Namespace.Class.Method() in file:line N`
///   - Ruby: `from file.rb:N:in 'method'`
pub struct StackTraceDetector;

// ─── Language-specific patterns ─────────────────────────────────

static PYTHON_TRACEBACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^Traceback \(most recent call last\):").unwrap()
});

static PYTHON_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s+File ".+", line \d+"#).unwrap()
});

static JS_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+at\s+.+\(.+:\d+:\d+\)").unwrap()
});

static JS_FRAME_ANON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+at\s+.+:\d+:\d+").unwrap()
});

static JAVA_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+at\s+[\w$.]+\([\w.]+:\d+\)").unwrap()
});

static RUST_PANIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^thread '.*' panicked at").unwrap()
});

static RUST_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+\d+:\s+\S+").unwrap()
});

static GO_GOROUTINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^goroutine \d+ \[").unwrap()
});

static GO_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\S+\.\S+\(").unwrap()
});

static DOTNET_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+at\s+[\w.]+\(.*\)\s+in\s+").unwrap()
});

static RUBY_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+from\s+.+:\d+:in\s+").unwrap()
});

/// Matches generic exception/error opening lines.
static ERROR_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\w+\.)*\w*(Error|Exception|Panic|FATAL|panic)\b").unwrap()
});

impl Detector for StackTraceDetector {
    fn detect(&self, lines: &[String]) -> f64 {
        if lines.is_empty() {
            return 0.0;
        }

        let mut frame_count = 0;
        let mut has_error_header = false;
        let total = lines.len();

        for line in lines {
            let s = line.as_str();

            // Headers / error lines.
            if PYTHON_TRACEBACK.is_match(s)
                || RUST_PANIC.is_match(s)
                || GO_GOROUTINE.is_match(s)
                || ERROR_LINE.is_match(s)
            {
                has_error_header = true;
                continue;
            }

            // Stack frames.
            if PYTHON_FRAME.is_match(s)
                || JS_FRAME.is_match(s)
                || JS_FRAME_ANON.is_match(s)
                || JAVA_FRAME.is_match(s)
                || RUST_FRAME.is_match(s)
                || GO_FRAME.is_match(s)
                || DOTNET_FRAME.is_match(s)
                || RUBY_FRAME.is_match(s)
            {
                frame_count += 1;
            }
        }

        // Need at least 2 frames and some kind of error header.
        if has_error_header && frame_count >= 2 {
            return 0.92;
        }

        // Frames without header (truncated stack trace).
        if frame_count >= 3 {
            #[allow(clippy::cast_precision_loss)]
            let ratio = f64::from(frame_count) / total as f64;
            if ratio > 0.4 {
                return 0.75;
            }
        }

        // Error header alone (single-line error).
        if has_error_header && frame_count >= 1 {
            return 0.6;
        }

        0.0
    }

    fn format(&self) -> Format {
        Format::StackTrace
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_python_traceback() {
        let lines = vec![
            "Traceback (most recent call last):".into(),
            r#"  File "/app/main.py", line 42, in handler"#.into(),
            "    return process(data)".into(),
            r#"  File "/app/process.py", line 15, in process"#.into(),
            "    raise ValueError(\"invalid input\")".into(),
            "ValueError: invalid input".into(),
        ];
        assert!(StackTraceDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_javascript_error() {
        let lines = vec![
            "TypeError: Cannot read properties of undefined (reading 'map')".into(),
            "    at processItems (/app/src/handler.js:42:15)".into(),
            "    at Object.<anonymous> (/app/src/index.js:10:3)".into(),
            "    at Module._compile (node:internal/modules/cjs/loader:1198:14)".into(),
        ];
        assert!(StackTraceDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_java_stack_trace() {
        let lines = vec![
            "java.lang.NullPointerException: Cannot invoke method on null".into(),
            "    at com.example.Service.process(Service.java:42)".into(),
            "    at com.example.Controller.handle(Controller.java:18)".into(),
            "    at org.springframework.web.servlet.DispatcherServlet.doDispatch(DispatcherServlet.java:1067)".into(),
        ];
        assert!(StackTraceDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_rust_panic() {
        let lines = vec![
            "thread 'main' panicked at 'index out of bounds: the len is 3 but the index is 5', src/main.rs:42:10".into(),
            "stack backtrace:".into(),
            "   0: std::panicking::begin_panic_handler".into(),
            "   1: core::panicking::panic_fmt".into(),
            "   2: core::panicking::panic_bounds_check".into(),
            "   3: prezzy::main".into(),
        ];
        assert!(StackTraceDetector.detect(&lines) > 0.9);
    }

    #[test]
    fn detects_go_panic() {
        let lines = vec![
            "goroutine 1 [running]:".into(),
            "main.handler(0xc0000b2000)".into(),
            "	/app/main.go:42 +0x1a4".into(),
            "main.main()".into(),
            "	/app/main.go:15 +0x25".into(),
        ];
        assert!(StackTraceDetector.detect(&lines) > 0.7);
    }

    #[test]
    fn rejects_plain_text() {
        let lines = vec![
            "hello world".into(),
            "this is normal output".into(),
            "no stack traces here".into(),
        ];
        assert!(StackTraceDetector.detect(&lines) < 0.1);
    }

    #[test]
    fn rejects_log_lines() {
        let lines = vec![
            "2024-01-15 10:30:45 INFO Server started on port 8080".into(),
            "2024-01-15 10:30:46 DEBUG Processing request".into(),
        ];
        assert!(StackTraceDetector.detect(&lines) < 0.1);
    }
}
