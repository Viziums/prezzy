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
