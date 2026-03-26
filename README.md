# prezzy

Make any CLI output beautiful. Zero config. Just pipe.

```sh
command | prezzy
```

## Install

```sh
cargo install prezzy
```

## What it does

Prezzy auto-detects the format of piped CLI output and applies syntax highlighting, structural formatting, and color.

```sh
curl -s https://api.github.com/users/octocat | prezzy   # JSON
docker compose logs | prezzy                              # Logs
git diff | prezzy                                         # Diffs
cat data.csv | prezzy                                     # Tables
env | prezzy                                              # Key=Value
```

## Supported formats

- JSON / NDJSON
- Log lines (syslog, structured, common formats)
- Diffs / patches
- CSV / TSV
- YAML
- XML / HTML
- Key=Value
- Markdown
- Stack traces
- Whitespace-aligned tables
- Plain text (URL highlighting, smart wrapping)

## Flags

```
prezzy [OPTIONS] [FILE]

Options:
  -f, --format <FORMAT>  Force a specific format
  -t, --theme <THEME>    Color theme [default: default]
      --color <MODE>     Color mode: auto, always, never [default: auto]
  -w, --width <COLS>     Override terminal width
  -h, --help             Print help
  -V, --version          Print version
```

## Standards

- Respects [`NO_COLOR`](https://no-color.org)
- Respects `FORCE_COLOR`
- Safe in pipelines (no color when stdout is not a TTY)
- Handles broken pipes gracefully (`cmd | prezzy | head`)

## License

MIT
