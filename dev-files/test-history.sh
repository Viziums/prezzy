#!/usr/bin/env bash
# =============================================================================
# History Feature Test Suite
# Run: bash dev-files/test-history.sh
# =============================================================================

set -euo pipefail

EXE="./target/release/prezzy.exe"
PASS=0
FAIL=0
TOTAL=0

pass() { PASS=$((PASS+1)); TOTAL=$((TOTAL+1)); echo "  [PASS] $1"; }
fail() { FAIL=$((FAIL+1)); TOTAL=$((TOTAL+1)); echo "  [FAIL] $1: $2"; }

assert_contains() {
    local name="$1" needle="$2" output="$3"
    if echo "$output" | grep -qF -- "$needle"; then pass "$name"
    else fail "$name" "expected '$needle'"; fi
}

assert_not_contains() {
    local name="$1" needle="$2" output="$3"
    if echo "$output" | grep -qF -- "$needle"; then fail "$name" "unexpected '$needle'"
    else pass "$name"; fi
}

assert_exit() {
    local name="$1" expected="$2"
    shift 2
    eval "$@" >/dev/null 2>&1
    local actual=$?
    if [[ "$actual" == "$expected" ]]; then pass "$name"
    else fail "$name" "exit $actual, expected $expected"; fi
}

echo "============================================"
echo "  HISTORY TESTS"
echo "============================================"

# ── CLI Help ─────────────────────────────────────────────────────────────────

echo ""
echo "--- CLI Help ---"

OUT=$($EXE history --help)
assert_contains "Help shows --top" "--top" "$OUT"
assert_contains "Help shows --failed" "--failed" "$OUT"
assert_contains "Help shows --slow" "--slow" "$OUT"
assert_contains "Help shows --search" "--search" "$OUT"
assert_contains "Help shows --today" "--today" "$OUT"
assert_contains "Help shows --week" "--week" "$OUT"
assert_contains "Help shows --dir" "--dir" "$OUT"
assert_contains "Help shows --export" "--export" "$OUT"
assert_contains "Help shows --clear" "--clear" "$OUT"
assert_contains "Help shows --stats" "--stats" "$OUT"
assert_contains "Help shows -n" "-n" "$OUT"

# ── Clear existing history for clean tests ───────────────────────────────────

echo ""
echo "--- Setup (clear history) ---"

$EXE history --clear >/dev/null 2>&1 || true
pass "History cleared"

# ── Populate history via shell session ───────────────────────────────────────

echo ""
echo "--- Populate via shell session ---"

python -c "
import subprocess, time, threading

proc = subprocess.Popen(
    ['$EXE', 'shell'],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
)
# Drain stdout in background to prevent blocking.
def drain():
    while proc.stdout.read(1): pass
threading.Thread(target=drain, daemon=True).start()

time.sleep(3)
commands = [
    'echo hello',
    'echo world',
    'ls',
    'echo hello',         # duplicate — tests --top frequency
    'ls /nonexistent 2>&1',  # exit code != 0
    # Note: space-prefixed commands are skipped by prezzy's should_skip(),
    # but bash's \$BASH_COMMAND strips the leading space, so the E marker
    # doesn't see it. This is a bash limitation — the Rust-side logic is
    # tested in unit tests.
    'echo done',
    'exit',
]
for cmd in commands:
    proc.stdin.write((cmd + '\n').encode())
    proc.stdin.flush()
    time.sleep(1.5)
time.sleep(2)
proc.kill()
" 2>/dev/null

pass "Shell session completed"

# ── Basic queries ────────────────────────────────────────────────────────────

echo ""
echo "--- Basic Queries ---"

OUT=$($EXE history -n 50)
assert_contains "Recent shows echo hello" "echo hello" "$OUT"
assert_contains "Recent shows echo world" "echo world" "$OUT"
assert_contains "Recent shows ls" "ls" "$OUT"
# Space-prefix skip is tested in unit tests (bash strips leading space from $BASH_COMMAND).

OUT=$($EXE history --stats)
assert_contains "Stats total" "Total commands:" "$OUT"
assert_contains "Stats unique" "Unique commands:" "$OUT"
assert_contains "Stats failed" "Failed commands:" "$OUT"
assert_contains "Stats success rate" "Success rate:" "$OUT"
assert_contains "Stats avg duration" "Avg duration:" "$OUT"

# ── Filtered queries ─────────────────────────────────────────────────────────

echo ""
echo "--- Filtered Queries ---"

OUT=$($EXE history --failed)
assert_contains "Failed shows ls /nonexistent" "ls /nonexistent" "$OUT"
assert_contains "Failed shows exit code" "exit 2" "$OUT"
assert_not_contains "Failed hides successful commands" "echo hello" "$OUT"

OUT=$($EXE history --search echo)
assert_contains "Search 'echo' finds echo hello" "echo hello" "$OUT"
assert_contains "Search 'echo' finds echo world" "echo world" "$OUT"
assert_not_contains "Search 'echo' excludes ls" "ls /nonexistent" "$OUT"

OUT=$($EXE history --top 3)
assert_contains "Top 3 shows echo hello (most frequent)" "echo hello" "$OUT"

OUT=$($EXE history --slow -n 3)
assert_contains "Slow shows commands" "ms" "$OUT"

# ── Time filters ─────────────────────────────────────────────────────────────

echo ""
echo "--- Time Filters ---"

OUT=$($EXE history --today)
assert_contains "Today filter returns results" "echo" "$OUT"

OUT=$($EXE history --week)
assert_contains "Week filter returns results" "echo" "$OUT"

# ── Dir filter ───────────────────────────────────────────────────────────────

echo ""
echo "--- Directory Filter ---"

# Get the CWD from a known record
CWD=$($EXE history --export -n 1 | tail -1 | cut -d',' -f5)
if [[ -n "$CWD" ]]; then
    OUT=$($EXE history --dir "$CWD")
    assert_contains "Dir filter matches CWD" "echo" "$OUT"
else
    fail "Dir filter" "no CWD in export"
fi

# ── Export ───────────────────────────────────────────────────────────────────

echo ""
echo "--- CSV Export ---"

OUT=$($EXE history --export)
assert_contains "Export has CSV header" "command,timestamp,duration_ms" "$OUT"
assert_contains "Export has data rows" "echo hello" "$OUT"

OUT=$($EXE history --export --failed)
assert_contains "Export with filter" "ls /nonexistent" "$OUT"

# ── Limit ────────────────────────────────────────────────────────────────────

echo ""
echo "--- Limit ---"

OUT=$($EXE history -n 2 | wc -l)
if [[ "$OUT" -le 2 ]]; then
    pass "Limit -n 2 returns at most 2 lines"
else
    fail "Limit -n 2" "got $OUT lines"
fi

# ── Nested session guard ────────────────────────────────────────────────────

echo ""
echo "--- Session Guard ---"

OUT=$(PREZZY_SHELL=1 $EXE shell 2>&1 || true)
assert_contains "Nested session blocked" "already inside" "$OUT"

# ── PREZZY_NO_HISTORY ────────────────────────────────────────────────────────

echo ""
echo "--- No History Env Var ---"

# Clear, run with PREZZY_NO_HISTORY=1, check nothing recorded
$EXE history --clear >/dev/null 2>&1 || true

python -c "
import subprocess, time, threading, os

env = {**os.environ, 'PREZZY_NO_HISTORY': '1'}
proc = subprocess.Popen(
    ['$EXE', 'shell'],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    env=env,
)
def drain():
    while proc.stdout.read(1): pass
threading.Thread(target=drain, daemon=True).start()

time.sleep(2)
proc.stdin.write(b'echo should_not_record\nexit\n')
proc.stdin.flush()
time.sleep(3)
proc.kill()
" 2>/dev/null

OUT=$($EXE history --stats)
assert_contains "No history recorded when disabled" "Total commands:  0" "$OUT"

# ── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "============================================"
echo "  RESULTS: $PASS passed, $FAIL failed (of $TOTAL)"
echo "============================================"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
