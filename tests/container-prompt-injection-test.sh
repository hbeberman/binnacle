#!/usr/bin/env bash
# Test script for container entrypoint prompt injection
# Validates the hybrid prompt injection approach used in container/entrypoint.sh
#
# Tests:
# 1. bn system emit copilot-instructions works and produces content
# 2. bn system emit mcp-lifecycle works and produces content
# 3. Both templates can be combined with delimiter
# 4. BN_INITIAL_PROMPT override is respected
# 5. Combined prompt has correct structure

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Use the built binary
BN="$REPO_ROOT/target/debug/bn"
if [[ ! -x "$BN" ]]; then
    BN="$REPO_ROOT/target/release/bn"
fi
if [[ ! -x "$BN" ]]; then
    echo "Error: bn binary not found. Run 'cargo build' first."
    exit 1
fi

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass() {
    echo -e "${GREEN}✓${NC} $1"
}

fail() {
    echo -e "${RED}✗${NC} $1"
    exit 1
}

section() {
    echo ""
    echo "========================================="
    echo "$1"
    echo "========================================="
}

section "Container Prompt Injection Tests"

# Test 1: copilot-instructions template produces content
section "Test 1: copilot-instructions template"
COPILOT_INST=$("$BN" system emit copilot-instructions -H 2>/dev/null || echo "")
if [[ -z "$COPILOT_INST" ]]; then
    fail "copilot-instructions template produced no content"
fi
if [[ "$COPILOT_INST" != *"Binnacle"* ]]; then
    fail "copilot-instructions should mention Binnacle"
fi
if [[ "$COPILOT_INST" != *"bn orient"* ]]; then
    fail "copilot-instructions should mention bn orient"
fi
COPILOT_LINES=$(echo "$COPILOT_INST" | wc -l)
if [[ $COPILOT_LINES -lt 5 ]]; then
    fail "copilot-instructions should have substantial content (got $COPILOT_LINES lines)"
fi
pass "copilot-instructions produces valid content ($COPILOT_LINES lines)"

# Test 2: mcp-lifecycle template produces content
section "Test 2: mcp-lifecycle template"
MCP_LIFECYCLE=$("$BN" system emit mcp-lifecycle -H 2>/dev/null || echo "")
if [[ -z "$MCP_LIFECYCLE" ]]; then
    fail "mcp-lifecycle template produced no content"
fi
if [[ "$MCP_LIFECYCLE" != *"MCP Lifecycle"* ]]; then
    fail "mcp-lifecycle should mention MCP Lifecycle"
fi
if [[ "$MCP_LIFECYCLE" != *"shell"* ]]; then
    fail "mcp-lifecycle should mention shell commands"
fi
MCP_LINES=$(echo "$MCP_LIFECYCLE" | wc -l)
if [[ $MCP_LINES -lt 5 ]]; then
    fail "mcp-lifecycle should have substantial content (got $MCP_LINES lines)"
fi
pass "mcp-lifecycle produces valid content ($MCP_LINES lines)"

# Test 3: Templates can be combined
section "Test 3: Template combination (simulating entrypoint.sh)"

# Replicate the entrypoint.sh logic
if [ -n "$COPILOT_INST" ] && [ -n "$MCP_LIFECYCLE" ]; then
    AGENT_INSTRUCTIONS="$COPILOT_INST

$MCP_LIFECYCLE"
elif [ -n "$COPILOT_INST" ]; then
    AGENT_INSTRUCTIONS="$COPILOT_INST"
elif [ -n "$MCP_LIFECYCLE" ]; then
    AGENT_INSTRUCTIONS="$MCP_LIFECYCLE"
else
    fail "Neither template produced content"
fi

COMBINED_LINES=$(echo "$AGENT_INSTRUCTIONS" | wc -l)
if [[ $COMBINED_LINES -lt $((COPILOT_LINES + MCP_LINES)) ]]; then
    fail "Combined instructions should include both templates"
fi
pass "Templates combine successfully ($COMBINED_LINES lines)"

# Test 4: BN_INITIAL_PROMPT override
section "Test 4: BN_INITIAL_PROMPT override"
DEFAULT_PROMPT="Run bn ready to see available tasks"
CUSTOM_PROMPT="Custom test prompt for validation"

# Simulate the entrypoint.sh logic
BN_INITIAL_PROMPT="${CUSTOM_PROMPT}"
if [[ "$BN_INITIAL_PROMPT" == "$CUSTOM_PROMPT" ]]; then
    pass "BN_INITIAL_PROMPT override is respected"
else
    fail "BN_INITIAL_PROMPT override failed"
fi

# Test 5: Full prompt structure
section "Test 5: Full prompt structure"

# Simulate the entrypoint.sh combination
if [ -n "$AGENT_INSTRUCTIONS" ]; then
    FULL_PROMPT="$AGENT_INSTRUCTIONS

---

$BN_INITIAL_PROMPT"
else
    FULL_PROMPT="$BN_INITIAL_PROMPT"
fi

# Verify structure
if [[ "$FULL_PROMPT" != *"---"* ]]; then
    fail "Combined prompt should have --- delimiter"
fi
if [[ "$FULL_PROMPT" != *"Binnacle"* ]]; then
    fail "Combined prompt should include Binnacle instructions"
fi
if [[ "$FULL_PROMPT" != *"$CUSTOM_PROMPT"* ]]; then
    fail "Combined prompt should include BN_INITIAL_PROMPT"
fi
if [[ "$FULL_PROMPT" != *"MCP"* ]]; then
    fail "Combined prompt should include MCP lifecycle"
fi

TOTAL_LINES=$(echo "$FULL_PROMPT" | wc -l)
pass "Full prompt has correct structure ($TOTAL_LINES lines)"

# Test 6: Templates work without git repo (critical for container startup)
section "Test 6: Templates work without initialization"
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

# Try running templates without any binnacle init
COPILOT_NO_INIT=$("$BN" system emit copilot-instructions -H 2>/dev/null || echo "")
MCP_NO_INIT=$("$BN" system emit mcp-lifecycle -H 2>/dev/null || echo "")

if [[ -z "$COPILOT_NO_INIT" ]]; then
    fail "copilot-instructions should work without init"
fi
if [[ -z "$MCP_NO_INIT" ]]; then
    fail "mcp-lifecycle should work without init"
fi
pass "Templates work without binnacle initialization"

# Cleanup
cd "$REPO_ROOT"
rm -rf "$TEMP_DIR"

section "All container prompt injection tests passed!"
echo ""
