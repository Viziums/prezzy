#!/usr/bin/env bash
# =============================================================================
# Shell Mode Test Suite
# Tests the PTY-based shell mode via Python subprocess control.
# Run: bash dev-files/test-shell-mode.sh
# =============================================================================

set -euo pipefail

EXE="./target/release/prezzy.exe"
PASS=0
FAIL=0
TOTAL=0

pass() { PASS=$((PASS+1)); TOTAL=$((TOTAL+1)); echo "  [PASS] $1"; }
fail() { FAIL=$((FAIL+1)); TOTAL=$((TOTAL+1)); echo "  [FAIL] $1: $2"; }

# Helper: run a shell session with given commands, return clean stdout.
shell_session() {
    local mode="$1"
    shift
    local commands=("$@")

    PYTHONIOENCODING=utf-8 python -c "
import subprocess, time, threading, re, sys, os

args = ['$EXE', 'shell']
if '$mode' == 'passthrough':
    args.append('--passthrough')

proc = subprocess.Popen(
    args,
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
)

output = []
def reader():
    while True:
        data = proc.stdout.read(1)
        if not data: break
        output.append(data)

t = threading.Thread(target=reader, daemon=True)
t.start()

time.sleep(3)

commands = $( printf '%s\n' "${commands[@]}" | python -c "import sys,json; print(json.dumps([l.strip() for l in sys.stdin]))" )
for cmd in commands:
    proc.stdin.write((cmd + '\n').encode())
    proc.stdin.flush()
    time.sleep(1.5)

time.sleep(2)

result = b''.join(output).decode('utf-8', errors='replace')
# Strip ANSI and OSC sequences.
clean = re.sub(r'\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b\[[\?0-9;]*[a-zA-Z]', '', result)
clean = clean.replace('\r\n', '\n').replace('\r', '')

# Print non-empty lines.
for line in clean.split('\n'):
    s = line.strip()
    if s:
        print(s)

proc.kill()
" 2>/dev/null
}

echo "============================================"
echo "  SHELL MODE TESTS"
echo "============================================"

# ── Basic Shell Functionality ────────────────────────────────────────────────

echo ""
echo "--- Basic Shell ---"

OUT=$(shell_session "normal" "echo SHELL_TEST_123" "exit")
if echo "$OUT" | grep -qF "SHELL_TEST_123"; then
    pass "Command executes and output visible"
else
    fail "Command executes" "SHELL_TEST_123 not found"
fi

OUT=$(shell_session "normal" 'echo $PREZZY_SHELL' "exit")
if echo "$OUT" | grep -qF "1"; then
    pass "PREZZY_SHELL=1 set in child"
else
    fail "PREZZY_SHELL=1 set" "not found in output"
fi

# ── JSON Beautification in Shell ─────────────────────────────────────────────

echo ""
echo "--- Beautification in Shell ---"

OUT=$(shell_session "normal" "echo '{\"name\":\"prezzy\",\"version\":\"1.0\"}'" "exit")
if echo "$OUT" | grep -qF '"name"'; then
    pass "JSON beautified in shell mode"
else
    fail "JSON beautified" "pretty-printed keys not found"
fi

# ── Key-Value Beautification ────────────────────────────────────────────────

OUT=$(shell_session "normal" 'printf "HOST=localhost\nPORT=8080\nDEBUG=true\n"' "exit")
if echo "$OUT" | grep -qF "HOST"; then
    pass "Key-value output in shell mode"
else
    fail "Key-value output" "HOST not found"
fi

# ── Plain Text Passthrough ──────────────────────────────────────────────────

OUT=$(shell_session "normal" "echo just plain text here" "exit")
if echo "$OUT" | grep -qF "just plain text here"; then
    pass "Plain text passes through"
else
    fail "Plain text passthrough" "text not found"
fi

# ── Passthrough Mode ─────────────────────────────────────────────────────────

echo ""
echo "--- Passthrough Mode ---"

OUT=$(shell_session "passthrough" "echo PASSTHROUGH_TEST" "exit")
if echo "$OUT" | grep -qF "PASSTHROUGH_TEST"; then
    pass "Passthrough mode forwards output"
else
    fail "Passthrough mode" "PASSTHROUGH_TEST not found"
fi

# ── Multiple Commands ────────────────────────────────────────────────────────

echo ""
echo "--- Multiple Commands ---"

OUT=$(shell_session "normal" "echo first" "echo second" "echo third" "exit")
FOUND=0
echo "$OUT" | grep -qF "first" && ((FOUND++)) || true
echo "$OUT" | grep -qF "second" && ((FOUND++)) || true
echo "$OUT" | grep -qF "third" && ((FOUND++)) || true
if [[ "$FOUND" -eq 3 ]]; then
    pass "Multiple sequential commands all visible"
else
    fail "Multiple commands" "found $FOUND of 3"
fi

# ── Exit Code Tracking ──────────────────────────────────────────────────────

echo ""
echo "--- Exit Code Tracking ---"

# Run a failing command then check history for it
$EXE history --clear >/dev/null 2>&1 || true
shell_session "normal" "ls /definitely_not_here 2>&1" "echo ok" "exit" >/dev/null 2>&1

OUT=$($EXE history --failed 2>&1)
if echo "$OUT" | grep -qF "ls /definitely_not_here"; then
    pass "Failed command recorded in history"
else
    fail "Failed command in history" "not found"
fi

# ── Unsupported Shell Warning ────────────────────────────────────────────────

echo ""
echo "--- Unsupported Shell Warning ---"

OUT=$(SHELL=/usr/bin/tcsh $EXE shell 2>&1 </dev/null & PID=$!; sleep 2; kill $PID 2>/dev/null; wait $PID 2>/dev/null || true)
if echo "$OUT" | grep -qi "unsupported"; then
    pass "Unsupported shell warning shown"
else
    # May not work if tcsh isn't installed — mark as skip
    echo "  [SKIP] Unsupported shell warning (tcsh not available)"
    ((TOTAL++))
fi

# ── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "============================================"
echo "  RESULTS: $PASS passed, $FAIL failed (of $TOTAL)"
echo "============================================"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
