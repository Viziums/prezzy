# prezzy

Make any CLI output beautiful. Zero config. Just pipe.

```sh
command | prezzy
```

Prezzy auto-detects the format of piped CLI output and applies syntax highlighting, structural formatting, and color. JSON gets indented. Logs get level-colored. CSV becomes a table. Diffs get green/red. All automatically.

## Install

**Homebrew** (macOS/Linux)
```sh
brew install viziums/tap/prezzy
```

**Cargo** (Rust)
```sh
cargo install prezzy
```

**npm** (via binary wrapper)
```sh
npx prezzy
```

**Pre-built binaries**

Download from [GitHub Releases](https://github.com/viziums/prezzy/releases) for Linux, macOS (Intel + Apple Silicon), and Windows.

**Shell completions**
```sh
prezzy --completions=bash >> ~/.bashrc
prezzy --completions=zsh >> ~/.zshrc
prezzy --completions=fish > ~/.config/fish/completions/prezzy.fish
```

## Usage

```sh
# JSON - auto-indented and highlighted
curl -s https://api.github.com/users/octocat | prezzy

# Structured logs (NDJSON) - columnar view with colored levels
docker compose logs --format json | prezzy

# Plain text logs - timestamp dimmed, levels colored
tail -f /var/log/app.log | prezzy

# Diffs - green additions, red deletions
git diff | prezzy

# CSV / TSV - rendered as a bordered table
cat data.csv | prezzy

# Environment variables - aligned columns
env | prezzy

# YAML config - syntax highlighted
cat config.yaml | prezzy

# XML / HTML - tag and attribute coloring
cat pom.xml | prezzy

# Stack traces - error bold red, library frames dimmed
python script.py 2>&1 | prezzy

# Markdown - headings, code blocks, lists
cat README.md | prezzy

# Filter logs by level
docker compose logs | prezzy --level=warn

# Use a different theme
cat data.json | prezzy --theme=dracula

# Pipe through a pager
kubectl logs pod-name | prezzy --pager
```

## Supported Formats

| Format | Detection | What prezzy does |
|--------|-----------|------------------|
| JSON | `{` or `[` start | Indent, syntax highlight keys/values/types |
| NDJSON | JSON objects per line | Columnar: timestamp, level, message, extra fields |
| Log lines | Timestamp + level patterns | Dim timestamps, color levels by severity |
| Diff / Patch | `@@`, `---`, `+++` markers | Green adds, red removes, bold headers |
| Stack traces | Python, JS, Java, Rust, Go, C#, Ruby | Bold error, dim library frames, highlight your code |
| CSV / TSV | Consistent delimiters | Bordered table with header row |
| Key=Value | `KEY=value` pattern | Aligned columns |
| YAML | `key: value` patterns | Syntax highlight keys, values, types |
| XML / HTML | `<tag>` patterns | Highlight tags, attributes, values |
| Markdown | `#` headings, fences, lists | Bold headings, colored markers, code blocks |
| Plain text | Fallback | Passthrough (no corruption) |

## Themes

```sh
prezzy --list-themes
```

Built-in: `default`, `monokai`, `dracula`, `solarized-dark`, `solarized-light`, `nord`, `gruvbox`

Set a default theme in `~/.config/prezzy/config.toml`:
```toml
theme = "dracula"
```

## Configuration

Create `~/.config/prezzy/config.toml`:

```toml
# Default color theme
theme = "default"

# Use ASCII box-drawing (for screen readers or limited terminals)
ascii = false

# Default log level filter
# level = "info"
```

CLI flags always override config file values.

## Flags

```
prezzy [OPTIONS] [FILE]

Options:
  -f, --format <FORMAT>    Force a specific format
  -t, --theme <THEME>      Color theme [default: default]
      --color <MODE>       auto, always, never [default: auto]
  -w, --width <COLS>       Override terminal width
  -l, --level <LEVEL>      Filter logs by minimum level
      --ascii              Use ASCII box-drawing characters
      --pager              Pipe through less
      --list-themes        List available themes
  -h, --help               Print help
  -V, --version            Print version
```

## Standards

- Respects [`NO_COLOR`](https://no-color.org) and `FORCE_COLOR`
- Safe in pipelines -- no escape codes when stdout is not a TTY
- Handles broken pipes gracefully (`prezzy | head`)
- Never modifies input when it can't detect the format

## License

MIT
