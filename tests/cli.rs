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
