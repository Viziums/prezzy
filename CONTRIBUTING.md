# Contributing to prezzy

Thanks for your interest in contributing to prezzy! Here's how to get started.

## Development setup

```sh
git clone https://github.com/viziums/prezzy.git
cd prezzy
cargo build
cargo test
```

## Running locally

```sh
echo '{"hello":"world"}' | cargo run
echo '{"hello":"world"}' | cargo run -- --theme=dracula
```

## Project structure

```
src/
  main.rs           Entry point, pager, CLI dispatch
  lib.rs            Core pipeline
  cli/mod.rs        Argument parsing (clap)
  config/mod.rs     Config file loading
  detect/           Format detectors (one file per format)
  render/           Format renderers (one file per format)
  terminal/         Terminal capability detection
  theme/mod.rs      Built-in color themes
  input/mod.rs      Input stream with peek buffer
tests/
  cli.rs            Integration tests
  fixtures/         Sample input files
```

## Adding a new format

1. Create `src/detect/myformat.rs` implementing the `Detector` trait
2. Create `src/render/myformat.rs` implementing the `Renderer` trait
3. Add the format to `Format` enum in `src/detect/mod.rs`
4. Register the detector in `detect_format()` in `src/detect/mod.rs`
5. Register the renderer in `renderer_for()` in `src/render/mod.rs`
6. Add test fixtures in `tests/fixtures/myformat/`
7. Add integration tests in `tests/cli.rs`

## Adding a new theme

Add a new constructor method to `Theme` in `src/theme/mod.rs` and register it in `by_name()` and `THEME_NAMES`.

## Code standards

- `cargo clippy --all-targets` must pass with zero warnings (pedantic + nursery)
- `cargo test --all-targets` must pass
- `unsafe` code is forbidden (`#[forbid(unsafe_code)]`)
- No unnecessary dependencies -- each new crate needs justification
- Prefer streaming over buffering (line-by-line when possible)
- Never crash on malformed input -- fall back to plain text

## Commit messages

Follow conventional commits:

- `feat:` new feature
- `fix:` bug fix
- `refactor:` code change that doesn't add/fix
- `test:` adding tests
- `chore:` build, deps, CI
- `docs:` documentation

Keep messages short and descriptive.

## Pull requests

- One logical change per PR
- Include tests for new functionality
- Update README if adding user-facing features
- Run `cargo clippy` and `cargo test` before submitting

## Reporting bugs

Use the [bug report template](https://github.com/viziums/prezzy/issues/new?template=bug_report.md). Include:

- prezzy version (`prezzy --version`)
- OS and terminal
- Input that triggers the bug (or a minimal reproduction)
- Expected vs actual output

## Requesting formats

Use the [format request template](https://github.com/viziums/prezzy/issues/new?template=format_request.md). Include:

- Example input (raw text)
- What you'd expect prezzy to do with it
- How common this format is in your workflow
