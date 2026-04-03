#!/usr/bin/env bash
# =============================================================================
# Pipe Mode Test Suite
# Run: bash dev-files/test-pipe-mode.sh
# =============================================================================

set -euo pipefail

EXE="./target/release/prezzy.exe"
PASS=0
FAIL=0
TOTAL=0

pass() { PASS=$((PASS+1)); TOTAL=$((TOTAL+1)); echo "  [PASS] $1"; }
fail() { FAIL=$((FAIL+1)); TOTAL=$((TOTAL+1)); echo "  [FAIL] $1: $2"; }

run() {
    local name="$1"
    shift
    local output
    output=$(eval "$@" 2>&1) || true
    echo "$output"
}

assert_contains() {
    local name="$1" needle="$2" output="$3"
    if echo "$output" | grep -qF -- "$needle"; then
        pass "$name"
    else
        fail "$name" "expected '$needle' in output"
    fi
}

assert_not_contains() {
    local name="$1" needle="$2" output="$3"
    if echo "$output" | grep -qF -- "$needle"; then
        fail "$name" "unexpected '$needle' in output"
    else
        pass "$name"
    fi
}

assert_exit() {
    local name="$1" expected="$2"
    shift 2
    eval "$@" >/dev/null 2>&1
    local actual=$?
    if [[ "$actual" == "$expected" ]]; then
        pass "$name"
    else
        fail "$name" "exit $actual, expected $expected"
    fi
}

echo "============================================"
echo "  PIPE MODE TESTS"
echo "============================================"

# ── Format Detection ─────────────────────────────────────────────────────────

echo ""
echo "--- Format Detection ---"

OUT=$(echo '{"name":"prezzy","version":"1.0"}' | $EXE)
assert_contains "JSON detected and pretty-printed" '"name": "prezzy"' "$OUT"

OUT=$(printf '{"a":1}\n{"b":2}\n{"c":3}\n' | $EXE)
assert_contains "NDJSON detected" "a=1" "$OUT"

OUT=$(printf 'name,age,city\nAlice,30,NYC\nBob,25,London\n' | $EXE)
assert_contains "CSV detected - header" "name" "$OUT"
assert_contains "CSV detected - border" "──" "$OUT"

OUT=$(printf 'name\tage\tcity\nAlice\t30\tNYC\n' | $EXE)
assert_contains "TSV detected" "Alice" "$OUT"

OUT=$(printf 'col1;col2;col3\na;b;c\nd;e;f\n' | $EXE)
assert_contains "Semicolon CSV detected" "col1" "$OUT"
assert_contains "Semicolon CSV multi-column" "col2" "$OUT"

OUT=$(printf 'name|age|city\nAlice|30|NYC\n' | $EXE)
assert_contains "Pipe CSV detected" "Alice" "$OUT"

OUT=$(printf '%s\n' '--- a/file' '+++ b/file' '@@ -1,3 +1,4 @@' ' hello' '+world' ' end' | $EXE)
assert_contains "Diff detected" "+world" "$OUT"

OUT=$(printf '2024-01-15 INFO started\n2024-01-15 WARN slow\n2024-01-15 ERROR crash\n' | $EXE)
assert_contains "Log lines detected" "ERROR" "$OUT"

OUT=$(printf 'DATABASE_URL=postgres://localhost\nPORT=8080\nDEBUG=false\n' | $EXE)
assert_contains "Key-value detected" "DATABASE_URL" "$OUT"

OUT=$(printf 'name: prezzy\nversion: 1.0\nfeatures:\n  - json\n  - yaml\n' | $EXE)
assert_contains "YAML detected" "name:" "$OUT"

OUT=$(printf '<?xml version="1.0"?>\n<root><item>hello</item></root>\n' | $EXE)
assert_contains "XML detected" "<root>" "$OUT"

OUT=$(printf '# Hello\n\nThis is **bold**.\n\n- item 1\n- item 2\n' | $EXE)
assert_contains "Markdown detected" "Hello" "$OUT"

OUT=$(printf 'Traceback (most recent call last):\n  File "app.py", line 42, in main\n    process()\nKeyError: key\n' | $EXE)
assert_contains "Python stacktrace detected" "Traceback" "$OUT"

OUT=$(printf 'TypeError: Cannot read\n    at handler (/app/src/handler.js:42:15)\n    at Router (/app/src/router.js:18:5)\n' | $EXE)
assert_contains "JS stacktrace detected" "TypeError" "$OUT"

# ── Edge Cases ───────────────────────────────────────────────────────────────

echo ""
echo "--- Edge Cases ---"

OUT=$(echo "" | $EXE)
assert_exit "Empty input exits 0" 0 'echo "" | '"$EXE"

OUT=$(printf '' | $EXE)
assert_exit "Truly empty stdin exits 0" 0 "printf '' | $EXE"

OUT=$(echo '{"incomplete": true' | $EXE)
assert_contains "Truncated JSON passthrough" '{"incomplete": true' "$OUT"

OUT=$(printf 'line1\nline2\nline3' | $EXE)
assert_contains "No trailing newline handled" "line3" "$OUT"

OUT=$(echo 'not json {invalid' | $EXE)
assert_contains "Invalid format passthrough" "not json" "$OUT"

OUT=$(printf '\n\n\n\n\n' | $EXE)
assert_exit "Only newlines exits 0" 0 "printf '\n\n\n\n\n' | $EXE"

OUT=$(printf '\t\t\ttabs\t\t\n' | $EXE)
assert_contains "Tab-only input handled" "tabs" "$OUT"

# ── Repeated Line Collapsing ─────────────────────────────────────────────────

echo ""
echo "--- Repeated Line Collapsing ---"

OUT=$(printf 'AAA\nAAA\nAAA\nAAA\nAAA\nBBB\n' | $EXE)
assert_contains "Repeated lines collapsed" "repeated 4 times" "$OUT"
assert_contains "Non-repeated line preserved" "BBB" "$OUT"

OUT=$(printf 'one\ntwo\nthree\n' | $EXE)
assert_not_contains "No collapse when no repeats" "repeated" "$OUT"

OUT=$(printf 'X\nX\nY\nY\nY\nZ\n' | $EXE)
assert_contains "Multiple collapse groups" "repeated 1 time" "$OUT"
assert_contains "Second collapse group" "repeated 2 times" "$OUT"

# ── Unicode & Special Characters ─────────────────────────────────────────────

echo ""
echo "--- Unicode & Special Characters ---"

OUT=$(printf '{"emoji":"🎨","jp":"日本語"}\n' | $EXE)
assert_contains "Unicode in JSON" "emoji" "$OUT"

OUT=$(printf 'name,city\n"O'\''Brien","NYC"\n"Müller","München"\n' | $EXE)
assert_contains "Special chars in CSV" "Müller" "$OUT"

# ── CLI Flags ────────────────────────────────────────────────────────────────

echo ""
echo "--- CLI Flags ---"

assert_exit "--help exits 0" 0 "$EXE --help"
assert_exit "--version exits 0" 0 "$EXE --version"
assert_exit "shell --help exits 0" 0 "$EXE shell --help"
assert_exit "history --help exits 0" 0 "$EXE history --help"

OUT=$($EXE --list-themes)
assert_contains "List themes" "dracula" "$OUT"
assert_contains "List themes has nord" "nord" "$OUT"

OUT=$(echo '{"a":1}' | $EXE --format plain)
assert_contains "Force plain format" '{"a":1}' "$OUT"

OUT=$(echo '{"a":1}' | $EXE --format json)
assert_contains "Force json format" '"a": 1' "$OUT"

OUT=$(echo 'test' | $EXE --format stacktrace)
assert_contains "Force stacktrace format" "test" "$OUT"

OUT=$(cat demo/samples/team.csv | $EXE --ascii)
assert_contains "ASCII box drawing" "+-" "$OUT"
assert_not_contains "No unicode borders in ASCII mode" "─" "$OUT"

OUT=$($EXE nonexistent_file.json 2>&1 || true)
assert_contains "Missing file error" "cannot open" "$OUT"

OUT=$($EXE demo/samples/api.json)
assert_contains "File argument mode" '"name": "prezzy"' "$OUT"

# ── PREZZY_ASCII env var ─────────────────────────────────────────────────────

echo ""
echo "--- Environment Variables ---"

OUT=$(export PREZZY_ASCII=1 && printf 'a,b\n1,2\n' | $EXE; unset PREZZY_ASCII)
assert_contains "PREZZY_ASCII=1 works" "+-" "$OUT"

OUT=$(export PREZZY_ASCII=yes && printf 'a,b\n1,2\n' | $EXE; unset PREZZY_ASCII)
assert_contains "PREZZY_ASCII=yes works" "+-" "$OUT"

OUT=$(export PREZZY_ASCII=0 && printf 'a,b\n1,2\n' | $EXE; unset PREZZY_ASCII)
assert_contains "PREZZY_ASCII=0 uses unicode" "─" "$OUT"

# ── Watch Mode ───────────────────────────────────────────────────────────────

echo ""
echo "--- Watch Mode ---"

OUT=$(printf '{"a":1}\n{"b":2}\n' | $EXE --watch)
assert_contains "Watch mode renders" "a=" "$OUT"

OUT=$(printf '2024-01-15 INFO ok\n2024-01-15 ERROR bad\n' | $EXE --watch)
assert_contains "Watch mode logs" "ERROR" "$OUT"

OUT=$(printf 'plain line\n' | $EXE -W)
assert_contains "Watch short flag -W" "plain line" "$OUT"

# ── Stress Tests ─────────────────────────────────────────────────────────────

echo ""
echo "--- Stress Tests ---"

OUT=$(python -c "print('A' * 200000)" | $EXE | wc -c)
if [[ "$OUT" -gt 199000 ]]; then
    pass "200K char single line preserved"
else
    fail "200K char single line preserved" "got $OUT chars"
fi

OUT=$(python -c "
for i in range(10000):
    print(f'line {i}')
" | $EXE | wc -l)
if [[ "$OUT" -eq 10000 ]]; then
    pass "10K lines all preserved"
else
    fail "10K lines all preserved" "got $OUT lines"
fi

OUT=$(python -c "
import json
print(json.dumps({'key'+str(i): 'val'+str(i) for i in range(500)}))
" | $EXE | wc -l)
if [[ "$OUT" -gt 100 ]]; then
    pass "Large JSON (500 keys) renders"
else
    fail "Large JSON (500 keys) renders" "got $OUT lines"
fi

# ── Level Filtering ──────────────────────────────────────────────────────────

echo ""
echo "--- Level Filtering ---"

OUT=$(cat demo/samples/logs.ndjson | $EXE --level warn)
assert_contains "Level warn shows WARN" "WARN" "$OUT"
assert_contains "Level warn shows ERROR" "ERROR" "$OUT"
assert_not_contains "Level warn hides INFO" "INFO" "$OUT"

OUT=$(cat demo/samples/logs.ndjson | $EXE --level error)
assert_contains "Level error shows ERROR" "ERROR" "$OUT"
assert_not_contains "Level error hides WARN" "WARN" "$OUT"

# ── Shell Completions ────────────────────────────────────────────────────────

echo ""
echo "--- Shell Completions ---"

OUT=$($EXE --completions bash | head -1)
assert_contains "Bash completions" "_prezzy" "$OUT"

OUT=$($EXE --completions zsh | head -1)
assert_contains "Zsh completions" "compdef" "$OUT"

OUT=$($EXE --completions fish | head -3)
assert_contains "Fish completions" "prezzy" "$OUT"

# ── Pipe Safety ──────────────────────────────────────────────────────────────

echo ""
echo "--- Pipe Safety ---"

OUT=$(echo '{"a":1}' | $EXE | head -1)
assert_exit "Pipe to head (broken pipe)" 0 "echo '{\"a\":1}' | $EXE | head -1"

# ── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "============================================"
echo "  RESULTS: $PASS passed, $FAIL failed (of $TOTAL)"
echo "============================================"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
