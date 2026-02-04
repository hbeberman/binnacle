#!/usr/bin/env bash
# Test suite for bn-agent drain mode functionality
# Tests the --drain flag, signal handling, and idle sleep behavior

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BN_AGENT="$REPO_ROOT/scripts/bn-agent"

# Color output for better readability
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

pass() {
    echo -e "${GREEN}✓${NC} $1"
}

fail() {
    echo -e "${RED}✗${NC} $1"
    exit 1
}

info() {
    echo -e "${YELLOW}ℹ${NC} $1"
}

section() {
    echo ""
    echo "========================================="
    echo "$1"
    echo "========================================="
}

# =============================================================================
# Test 1: --drain --once produces error (mutually exclusive flags)
# =============================================================================
section "Test 1: --drain --once Are Mutually Exclusive"

output=$("$BN_AGENT" --drain --once --host auto 2>&1 || true)
if echo "$output" | grep -q "mutually exclusive"; then
    pass "--drain --once produces error about mutual exclusivity"
else
    echo "Output: $output"
    fail "--drain --once should produce mutual exclusivity error"
fi

# Also test in reverse order
output=$("$BN_AGENT" --once --drain --host auto 2>&1 || true)
if echo "$output" | grep -q "mutually exclusive"; then
    pass "--once --drain (reverse order) also produces error"
else
    fail "--once --drain should also produce mutual exclusivity error"
fi

# =============================================================================
# Test 2: Drain mode documentation in usage
# =============================================================================
section "Test 2: Drain Mode Documented in Usage"

output=$("$BN_AGENT" --help 2>&1 || true)
if echo "$output" | grep -q -- "--drain"; then
    pass "--drain flag documented in help"
else
    fail "--drain flag not documented in help"
fi

if echo "$output" | grep -q "Drain Mode:"; then
    pass "Drain Mode section present in help"
else
    fail "Drain Mode section missing from help"
fi

if echo "$output" | grep -q "no_work_remaining"; then
    pass "Exit reason 'no_work_remaining' documented"
else
    fail "Exit reason 'no_work_remaining' not documented"
fi

if echo "$output" | grep -q "immediate_no_work"; then
    pass "Exit reason 'immediate_no_work' documented"
else
    fail "Exit reason 'immediate_no_work' not documented"
fi

if echo "$output" | grep -q "interrupted"; then
    pass "Exit reason 'interrupted' documented"
else
    fail "Exit reason 'interrupted' not documented"
fi

# =============================================================================
# Test 3: Environment variable BN_DRAIN_JSON documented
# =============================================================================
section "Test 3: BN_DRAIN_JSON Environment Variable"

if grep -q "BN_DRAIN_JSON" "$BN_AGENT"; then
    pass "BN_DRAIN_JSON variable exists in script"
else
    fail "BN_DRAIN_JSON variable missing from script"
fi

output=$("$BN_AGENT" --help 2>&1 || true)
if echo "$output" | grep -q "BN_DRAIN_JSON"; then
    pass "BN_DRAIN_JSON documented in help"
else
    fail "BN_DRAIN_JSON not documented in help"
fi

# =============================================================================
# Test 4: check_work_remaining function exists
# =============================================================================
section "Test 4: check_work_remaining Function"

if grep -q "check_work_remaining()" "$BN_AGENT"; then
    pass "check_work_remaining() function exists"
else
    fail "check_work_remaining() function missing"
fi

# Verify it queries bn ready
if grep -A 10 "check_work_remaining()" "$BN_AGENT" | grep -q "bn ready"; then
    pass "check_work_remaining() queries 'bn ready'"
else
    fail "check_work_remaining() doesn't query 'bn ready'"
fi

# Verify it counts both tasks and bugs
if grep -A 10 "check_work_remaining()" "$BN_AGENT" | grep -q "bug_count"; then
    pass "check_work_remaining() includes bug_count"
else
    fail "check_work_remaining() doesn't include bug_count"
fi

# =============================================================================
# Test 5: drain_complete function exists and produces correct output
# =============================================================================
section "Test 5: drain_complete Function"

if grep -q "drain_complete()" "$BN_AGENT"; then
    pass "drain_complete() function exists"
else
    fail "drain_complete() function missing"
fi

# Verify it outputs human-readable summary
if grep -A 20 "drain_complete()" "$BN_AGENT" | grep -q "No work remaining"; then
    pass "drain_complete() outputs human-readable summary"
else
    fail "drain_complete() doesn't output human-readable summary"
fi

# Verify it outputs JSON to stderr when BN_DRAIN_JSON=1
if grep -A 25 "drain_complete()" "$BN_AGENT" | grep -q '"status":"drained"'; then
    pass "drain_complete() outputs JSON with 'drained' status"
else
    fail "drain_complete() doesn't output correct JSON"
fi

if grep -A 25 "drain_complete()" "$BN_AGENT" | grep -q '"exit_reason"'; then
    pass "drain_complete() JSON includes exit_reason"
else
    fail "drain_complete() JSON missing exit_reason"
fi

# =============================================================================
# Test 6: Pre-flight check in drain mode
# =============================================================================
section "Test 6: Drain Mode Pre-flight Check"

# Verify script checks work before first iteration in drain mode
if grep -q "immediate_no_work" "$BN_AGENT"; then
    pass "Pre-flight check uses 'immediate_no_work' exit reason"
else
    fail "Pre-flight check doesn't use 'immediate_no_work' exit reason"
fi

# Verify pre-flight check happens before the loop
# Look for the pattern: check work → drain_complete → exit 0
if grep -B 5 "immediate_no_work" "$BN_AGENT" | grep -q "check_work_remaining"; then
    pass "Pre-flight check calls check_work_remaining()"
else
    fail "Pre-flight check doesn't call check_work_remaining()"
fi

# =============================================================================
# Test 7: Between-iteration check in drain mode
# =============================================================================
section "Test 7: Drain Mode Between-iteration Check"

# Verify script checks work between iterations
if grep -q "no_work_remaining" "$BN_AGENT"; then
    pass "Between-iteration check uses 'no_work_remaining' exit reason"
else
    fail "Between-iteration check doesn't use 'no_work_remaining' exit reason"
fi

# =============================================================================
# Test 8: Signal handlers for drain mode
# =============================================================================
section "Test 8: Signal Handlers for Drain Mode"

if grep -q "handle_drain_sigint" "$BN_AGENT"; then
    pass "Drain mode has SIGINT handler"
else
    fail "Drain mode missing SIGINT handler"
fi

if grep -q "handle_drain_sigterm" "$BN_AGENT"; then
    pass "Drain mode has SIGTERM handler"
else
    fail "Drain mode missing SIGTERM handler"
fi

# Verify handlers call drain_complete with 'interrupted' reason
if grep -A 5 "handle_drain_sigint()" "$BN_AGENT" | grep -q "interrupted"; then
    pass "SIGINT handler uses 'interrupted' exit reason"
else
    fail "SIGINT handler doesn't use 'interrupted' exit reason"
fi

if grep -A 5 "handle_drain_sigterm()" "$BN_AGENT" | grep -q "interrupted"; then
    pass "SIGTERM handler uses 'interrupted' exit reason"
else
    fail "SIGTERM handler doesn't use 'interrupted' exit reason"
fi

# =============================================================================
# Test 9: Normal auto mode idle sleep behavior
# =============================================================================
section "Test 9: Normal Auto Mode Idle Sleep"

if grep -q "wait_for_work()" "$BN_AGENT"; then
    pass "wait_for_work() function exists"
else
    fail "wait_for_work() function missing"
fi

if grep -q "IDLE_SLEEP_SECONDS" "$BN_AGENT"; then
    pass "IDLE_SLEEP_SECONDS variable defined"
else
    fail "IDLE_SLEEP_SECONDS variable missing"
fi

# Verify wait_for_work sleeps and checks work
if grep -A 10 "wait_for_work()" "$BN_AGENT" | grep -q "sleep"; then
    pass "wait_for_work() includes sleep"
else
    fail "wait_for_work() doesn't include sleep"
fi

# Verify normal mode (not --drain) calls wait_for_work
if grep -q 'DRAIN_MODE.*!=.*true.*wait_for_work' "$BN_AGENT" || \
   grep -B 3 "wait_for_work" "$BN_AGENT" | grep -q "DRAIN_MODE"; then
    pass "Normal mode calls wait_for_work() when no work"
else
    fail "Normal mode doesn't properly call wait_for_work()"
fi

# =============================================================================
# Test 10: Iteration counting in drain mode
# =============================================================================
section "Test 10: Iteration Counting"

if grep -q "ITERATION_COUNT" "$BN_AGENT"; then
    pass "ITERATION_COUNT variable used"
else
    fail "ITERATION_COUNT variable missing"
fi

if grep -q "DRAIN_START_TIME" "$BN_AGENT"; then
    pass "DRAIN_START_TIME variable used"
else
    fail "DRAIN_START_TIME variable missing"
fi

# Verify iteration count is incremented after each agent run
if grep -q 'ITERATION_COUNT=.*ITERATION_COUNT.*+.*1' "$BN_AGENT" || \
   grep -q 'ITERATION_COUNT=\$((ITERATION_COUNT + 1))' "$BN_AGENT"; then
    pass "ITERATION_COUNT is incremented in loop"
else
    fail "ITERATION_COUNT not incremented properly"
fi

# =============================================================================
# Test 11: Container mode also supports drain
# =============================================================================
section "Test 11: Container Mode Drain Support"

# Verify drain mode logic exists in container mode section (USE_CONTAINER=true)
# Look for DRAIN_MODE checks after the container mode check
if grep -A 200 'USE_CONTAINER.*==.*true' "$BN_AGENT" | grep -q "DRAIN_MODE"; then
    pass "Drain mode supported in container mode"
else
    fail "Drain mode not supported in container mode"
fi

# Verify both host and container modes have drain signal handlers
container_drain_handlers=$(grep -c "handle_drain_sig" "$BN_AGENT")
if [[ "$container_drain_handlers" -ge 4 ]]; then
    pass "Drain signal handlers defined for both modes (found $container_drain_handlers definitions)"
else
    fail "Drain signal handlers not defined for both modes (found $container_drain_handlers, expected 4+)"
fi

# =============================================================================
# Test 12: Exit codes
# =============================================================================
section "Test 12: Exit Codes"

# Verify SIGINT handler exits with 130 (128 + 2)
if grep -A 5 "handle_drain_sigint()" "$BN_AGENT" | grep -q "exit 130"; then
    pass "SIGINT handler exits with code 130"
else
    fail "SIGINT handler doesn't exit with code 130"
fi

# Verify SIGTERM handler exits with 143 (128 + 15)
if grep -A 5 "handle_drain_sigterm()" "$BN_AGENT" | grep -q "exit 143"; then
    pass "SIGTERM handler exits with code 143"
else
    fail "SIGTERM handler doesn't exit with code 143"
fi

# Verify normal drain completion exits with 0
# Check that exit 0 follows the drain_complete calls (within next 2 lines)
if grep -A 2 "drain_complete.*immediate_no_work\|drain_complete.*no_work_remaining" "$BN_AGENT" | grep -q "exit 0"; then
    pass "Normal drain completion exits with code 0"
else
    fail "Normal drain completion doesn't exit with code 0"
fi

# =============================================================================
# Summary
# =============================================================================
section "Summary"
echo -e "${GREEN}All drain mode tests passed!${NC}"
echo ""
echo "Test Coverage:"
echo "  ✓ --drain --once mutual exclusivity"
echo "  ✓ Drain mode documentation in usage"
echo "  ✓ BN_DRAIN_JSON environment variable"
echo "  ✓ check_work_remaining() function"
echo "  ✓ drain_complete() function and JSON output"
echo "  ✓ Pre-flight check (immediate_no_work)"
echo "  ✓ Between-iteration check (no_work_remaining)"
echo "  ✓ Signal handlers (SIGINT, SIGTERM)"
echo "  ✓ Normal auto mode idle sleep"
echo "  ✓ Iteration counting"
echo "  ✓ Container mode drain support"
echo "  ✓ Exit codes"
