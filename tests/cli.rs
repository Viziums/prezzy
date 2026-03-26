use assert_cmd::Command;
use predicates::prelude::*;

fn prezzy() -> Command {
    Command::cargo_bin("prezzy").unwrap()
}

// ─── Help & Version ─────────────────────────────────────────────

#[test]
fn shows_help() {
    prezzy()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("auto-detects the format"));
}

#[test]
fn shows_version() {
    prezzy()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

// ─── Passthrough Behavior ───────────────────────────────────────

#[test]
fn passthrough_plain_text() {
    prezzy()
        .arg("--color=never")
        .write_stdin("hello world\n")
        .assert()
        .success()
        .stdout("hello world\n");
}

#[test]
fn passthrough_empty_input() {
    prezzy()
        .arg("--color=never")
        .write_stdin("")
        .assert()
        .success()
        .stdout("");
}

#[test]
fn passthrough_multiline() {
    let input = "line one\nline two\nline three\n";
    prezzy()
        .arg("--color=never")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(input);
}

// ─── File Input ─────────────────────────────────────────────────

#[test]
fn reads_file_argument() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/plain/hello.txt")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello world"));
}

#[test]
fn errors_on_missing_file() {
    prezzy()
        .arg("nonexistent_file.txt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot open"));
}

// ─── JSON Detection ─────────────────────────────────────────────

#[test]
fn detects_and_formats_json() {
    prezzy()
        .arg("--color=never")
        .write_stdin(r#"{"name":"prezzy"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("\"prezzy\""));
}

#[test]
fn json_file_is_pretty_printed() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/json/simple_object.json")
        .assert()
        .success()
        // Pretty-printed JSON should have indentation.
        .stdout(predicate::str::contains("  \"name\""));
}

// ─── Format Override ────────────────────────────────────────────

#[test]
fn force_plain_format() {
    prezzy()
        .args(["--color=never", "--format=plain"])
        .write_stdin(r#"{"name":"prezzy"}"#)
        .assert()
        .success()
        // Should NOT be pretty-printed since we forced plain.
        .stdout(predicate::str::contains(r#"{"name":"prezzy"}"#));
}

// ─── NDJSON Detection ──────────────────────────────────────────

#[test]
fn detects_ndjson_and_extracts_fields() {
    let input = r#"{"level":"info","ts":"2024-01-15T10:30:45Z","msg":"started","port":8080}
{"level":"error","ts":"2024-01-15T10:30:46Z","msg":"failed","code":500}
"#;
    prezzy()
        .arg("--color=never")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("started"))
        .stdout(predicate::str::contains("failed"));
}

#[test]
fn ndjson_file_shows_structured_output() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/logs/ndjson.log")
        .assert()
        .success()
        .stdout(predicate::str::contains("server started"))
        .stdout(predicate::str::contains("connection refused"));
}

// ─── Log Detection ─────────────────────────────────────────────

#[test]
fn detects_log_lines() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/logs/app.log")
        .assert()
        .success()
        .stdout(predicate::str::contains("Failed to connect"))
        .stdout(predicate::str::contains("Retrying connection"));
}

// ─── Level Filtering ───────────────────────────────────────────

#[test]
fn level_filter_on_ndjson() {
    let input = r#"{"level":"debug","msg":"noisy"}
{"level":"info","msg":"normal"}
{"level":"error","msg":"critical"}
"#;
    prezzy()
        .args(["--color=never", "--level=warn"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("critical"))
        // debug and info should be filtered out
        .stdout(predicate::str::contains("noisy").not())
        .stdout(predicate::str::contains("normal").not());
}

#[test]
fn level_filter_on_plain_logs() {
    let input = "2024-01-15T10:30:45Z DEBUG pool stats\n\
                 2024-01-15T10:30:46Z ERROR connection refused\n\
                 2024-01-15T10:30:47Z INFO started\n";
    prezzy()
        .args(["--color=never", "--level=error"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("connection refused"))
        .stdout(predicate::str::contains("pool stats").not())
        .stdout(predicate::str::contains("started").not());
}

// ─── Diff Detection ────────────────────────────────────────────

#[test]
fn detects_diff_output() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/diff/simple.patch")
        .assert()
        .success()
        .stdout(predicate::str::contains("diff --git"))
        .stdout(predicate::str::contains("+use std::fs;"))
        .stdout(predicate::str::contains("-    println!(\"Hello, world!\");"));
}

#[test]
fn diff_from_stdin() {
    let input = "--- a/file.txt\n+++ b/file.txt\n@@ -1,2 +1,2 @@\n-old\n+new\n context\n";
    prezzy()
        .arg("--color=never")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("-old"))
        .stdout(predicate::str::contains("+new"));
}

#[test]
fn force_diff_format() {
    let input = "+added line\n-removed line\n context\n";
    prezzy()
        .args(["--color=never", "--format=diff"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("+added line"))
        .stdout(predicate::str::contains("-removed line"));
}

// ─── Stack Trace Detection ─────────────────────────────────────

#[test]
fn detects_python_traceback() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/stacktrace/python.txt")
        .assert()
        .success()
        .stdout(predicate::str::contains("Traceback"))
        .stdout(predicate::str::contains("JSONDecodeError"));
}

#[test]
fn detects_javascript_error() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/stacktrace/javascript.txt")
        .assert()
        .success()
        .stdout(predicate::str::contains("TypeError"))
        .stdout(predicate::str::contains("processItems"));
}

#[test]
fn detects_rust_panic() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/stacktrace/rust.txt")
        .assert()
        .success()
        .stdout(predicate::str::contains("panicked at"))
        .stdout(predicate::str::contains("prezzy::main"));
}

// ─── CSV Detection ─────────────────────────────────────────────

#[test]
fn detects_csv_and_renders_table() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/csv/data.csv")
        .assert()
        .success()
        // Table should have borders and aligned content.
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("│"));
}

#[test]
fn csv_from_stdin() {
    let input = "a,b,c\n1,2,3\n4,5,6\n";
    prezzy()
        .arg("--color=never")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("│"));
}

// ─── Key=Value Detection ───────────────────────────────────────

#[test]
fn detects_key_value() {
    let input = "HOME=/home/user\nPATH=/usr/bin\nSHELL=/bin/bash\nTERM=xterm\n";
    prezzy()
        .arg("--color=never")
        .write_stdin(input)
        .assert()
        .success()
        // Should align the = signs.
        .stdout(predicate::str::contains("HOME"))
        .stdout(predicate::str::contains("="));
}

// ─── YAML Detection ────────────────────────────────────────────

#[test]
fn detects_yaml() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/yaml/config.yaml")
        .assert()
        .success()
        .stdout(predicate::str::contains("server"))
        .stdout(predicate::str::contains("localhost"));
}

// ─── XML Detection ─────────────────────────────────────────────

#[test]
fn detects_xml() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/xml/config.xml")
        .assert()
        .success()
        .stdout(predicate::str::contains("<config>"))
        .stdout(predicate::str::contains("</config>"));
}

// ─── Markdown Detection ────────────────────────────────────────

#[test]
fn detects_markdown() {
    prezzy()
        .arg("--color=never")
        .arg("tests/fixtures/markdown/readme.md")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Prezzy"))
        .stdout(predicate::str::contains("- Auto-detect format"));
}
