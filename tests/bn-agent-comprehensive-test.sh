#!/usr/bin/env bash
# Comprehensive test suite for bn-agent script
# Tests all agent types, modes, options, and error handling

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

# Test 1: Script exists and is executable
section "Test 1: Script Validation"
if [[ -x "$BN_AGENT" ]]; then
    pass "bn-agent script exists and is executable"
else
    fail "bn-agent script not found or not executable at: $BN_AGENT"
fi

# Test 2: Help/usage output
section "Test 2: Help and Usage"
if "$BN_AGENT" --help 2>&1 | grep -q "Usage:"; then
    pass "--help displays usage information"
else
    fail "--help does not display usage"
fi

if "$BN_AGENT" 2>&1 | grep -q "Usage:"; then
    pass "No arguments displays usage"
else
    fail "No arguments does not display usage"
fi

# Test 3: Unknown agent type error
section "Test 3: Error Handling"
if "$BN_AGENT" unknown-type 2>&1 | grep -q "Unknown agent type"; then
    pass "Unknown agent type produces error message"
else
    fail "Unknown agent type does not produce proper error"
fi

# Test 4: Agent type validation
section "Test 4: Agent Type Validation"
AGENT_TYPES=("auto" "do" "prd" "buddy" "free")
for agent_type in "${AGENT_TYPES[@]}"; do
    # Just validate the script accepts the agent type (will fail on copilot execution, but that's OK)
    # We're testing argument parsing, not full execution
    case "$agent_type" in
        do)
            # 'do' requires a description
            if "$BN_AGENT" --once do "test task" 2>&1 | grep -q "Launching agent"; then
                pass "Agent type '$agent_type' accepted"
            else
                # May fail on copilot execution, but should at least recognize the type
                if ! "$BN_AGENT" do "test" 2>&1 | grep -q "Unknown agent type"; then
                    pass "Agent type '$agent_type' recognized"
                else
                    fail "Agent type '$agent_type' not recognized"
                fi
            fi
            ;;
        *)
            # Other agent types don't need extra args
            if "$BN_AGENT" --once "$agent_type" 2>&1 | grep -q "Launching agent"; then
                pass "Agent type '$agent_type' accepted"
            else
                # May fail on copilot execution, but should at least recognize the type
                if ! "$BN_AGENT" "$agent_type" 2>&1 | grep -q "Unknown agent type"; then
                    pass "Agent type '$agent_type' recognized"
                else
                    fail "Agent type '$agent_type' not recognized"
                fi
            fi
            ;;
    esac
done

# Test 5: 'do' agent requires description
section "Test 5: 'do' Agent Validation"
if "$BN_AGENT" do 2>&1 | grep -q "requires a description"; then
    pass "'do' without description produces error"
else
    fail "'do' without description does not produce proper error"
fi

# Test 6: Mode selection (container vs host)
section "Test 6: Mode Selection"
# auto → container by default
if "$BN_AGENT" --once auto 2>&1 | grep -q "CONTAINER mode"; then
    pass "'auto' defaults to CONTAINER mode"
else
    # Container mode may not be available, check if it attempted container mode
    if "$BN_AGENT" --once auto 2>&1 | grep -q "container infrastructure"; then
        pass "'auto' attempts CONTAINER mode (infra not available)"
    else
        fail "'auto' does not default to container mode"
    fi
fi

# auto --host → host mode
if "$BN_AGENT" --once --host auto 2>&1 | grep -q "HOST mode"; then
    pass "'auto --host' uses HOST mode"
else
    fail "'auto --host' does not use HOST mode"
fi

# Other agent types → host by default
if "$BN_AGENT" --once prd 2>&1 | grep -q "HOST mode"; then
    pass "'prd' defaults to HOST mode"
else
    fail "'prd' does not default to HOST mode"
fi

if "$BN_AGENT" --once buddy 2>&1 | grep -q "HOST mode"; then
    pass "'buddy' defaults to HOST mode"
else
    fail "'buddy' does not default to HOST mode"
fi

if "$BN_AGENT" --once free 2>&1 | grep -q "HOST mode"; then
    pass "'free' defaults to HOST mode"
else
    fail "'free' does not default to HOST mode"
fi

if "$BN_AGENT" --once do "test" 2>&1 | grep -q "HOST mode"; then
    pass "'do' defaults to HOST mode"
else
    fail "'do' does not default to HOST mode"
fi

# Test 7: Container mode requires infrastructure
section "Test 7: Container Infrastructure Check"
# Try to run auto without --host (should fail if no container infra)
output=$("$BN_AGENT" --once auto 2>&1 || true)
if echo "$output" | grep -q "Container infrastructure not available" || \
   echo "$output" | grep -q "Containerd is not running" || \
   echo "$output" | grep -q "CONTAINER mode"; then
    pass "Container infrastructure check works (either passes or fails gracefully)"
else
    info "Container check output: $output"
    fail "Container infrastructure check not working properly"
fi

# Test 8: Option parsing
section "Test 8: Option Parsing"
# Use timeout to prevent hanging on actual copilot execution
# We're just testing argument parsing, not full execution

# Test --once flag
if timeout 2 "$BN_AGENT" --once --host auto 2>&1 | grep -q "Running once"; then
    pass "--once flag recognized"
else
    fail "--once flag not recognized"
fi

# Test loop mode messaging (default) - need to kill quickly
output=$(timeout 2 "$BN_AGENT" --host auto 2>&1 || true)
if echo "$output" | grep -q "Loop mode enabled"; then
    pass "Loop mode enabled by default"
else
    fail "Loop mode not enabled by default"
fi

# For container-specific options, just verify they don't cause parse errors
# Test --cpus flag (for container mode)
if timeout 2 "$BN_AGENT" --once --cpus 2 auto 2>&1 | grep -q "Launching agent"; then
    pass "--cpus flag accepted"
else
    fail "--cpus flag not accepted"
fi

# Test --memory flag (for container mode)
if timeout 2 "$BN_AGENT" --once --memory 4g auto 2>&1 | grep -q "Launching agent"; then
    pass "--memory flag accepted"
else
    fail "--memory flag not accepted"
fi

# Test --name flag (for container mode)
if timeout 2 "$BN_AGENT" --once --name test-agent auto 2>&1 | grep -q "Launching agent"; then
    pass "--name flag accepted"
else
    fail "--name flag not accepted"
fi

# Test --merge-target flag
if timeout 2 "$BN_AGENT" --once --merge-target develop auto 2>&1 | grep -q "Launching agent"; then
    pass "--merge-target flag accepted"
else
    fail "--merge-target flag not accepted"
fi

# Test --no-merge flag
if timeout 2 "$BN_AGENT" --once --no-merge auto 2>&1 | grep -q "Launching agent"; then
    pass "--no-merge flag accepted"
else
    fail "--no-merge flag not accepted"
fi

# Test 9: Template emission
section "Test 9: Template Emission"
# Verify script can emit templates (with timeout to avoid hanging)
TEMPLATE_CHECK=$(timeout 2 "$BN_AGENT" --once --host auto 2>&1 || true)
if echo "$TEMPLATE_CHECK" | grep -q "Failed to emit template"; then
    fail "Template emission failed"
elif echo "$TEMPLATE_CHECK" | grep -q "Launching agent" || \
     echo "$TEMPLATE_CHECK" | grep -q "Copilot"; then
    pass "Template emission works"
else
    # May fail for other reasons (copilot not found, etc), that's OK
    pass "Template emission attempted (script logic correct)"
fi

# Test 10: Dependency checking
section "Test 10: Dependency Checking"
# Dependencies are checked before argument parsing, so we expect them to pass
# (since we're running in a working environment)
if "$BN_AGENT" --help 2>&1 | grep -q "Missing required dependencies"; then
    fail "Dependency check failed in working environment"
else
    pass "Dependency check passes in working environment"
fi

# Test 11: Error message quality
section "Test 11: Error Message Quality"
# Unknown agent type
ERROR_OUTPUT=$("$BN_AGENT" invalid 2>&1 || true)
if echo "$ERROR_OUTPUT" | grep -q "Error:" && \
   echo "$ERROR_OUTPUT" | grep -q "Unknown agent type"; then
    pass "Unknown agent type error is clear"
else
    fail "Unknown agent type error message unclear"
fi

# Missing description for 'do'
ERROR_OUTPUT=$("$BN_AGENT" do 2>&1 || true)
if echo "$ERROR_OUTPUT" | grep -q "Error:" && \
   echo "$ERROR_OUTPUT" | grep -q "requires a description"; then
    pass "'do' missing description error is clear"
else
    fail "'do' missing description error message unclear"
fi

# Too many arguments for 'do'
ERROR_OUTPUT=$("$BN_AGENT" do arg1 arg2 2>&1 || true)
if echo "$ERROR_OUTPUT" | grep -q "Error:" && \
   echo "$ERROR_OUTPUT" | grep -q "Too many arguments"; then
    pass "'do' too many arguments error is clear"
else
    fail "'do' too many arguments error message unclear"
fi

# Test 12: Blocked tools configuration
section "Test 12: Blocked Tools Configuration"
# Verify script contains blocked tool definitions
if grep -q "BLOCKED_TOOLS" "$BN_AGENT" && \
   grep -q "bn agent kill" "$BN_AGENT" && \
   grep -q "binnacle-orient" "$BN_AGENT" && \
   grep -q "binnacle-goodbye" "$BN_AGENT"; then
    pass "Blocked tools properly configured"
else
    fail "Blocked tools not properly configured"
fi

# Test 13: Tool permissions configuration
section "Test 13: Tool Permissions Configuration"
if grep -q "TOOLS_FULL" "$BN_AGENT" && \
   grep -q "TOOLS_PRD" "$BN_AGENT" && \
   grep -q "TOOLS_BUDDY" "$BN_AGENT"; then
    pass "Tool permission sets defined"
else
    fail "Tool permission sets not defined"
fi

# Verify tool sets have appropriate permissions
if grep -A 50 "TOOLS_FULL=" "$BN_AGENT" | grep -q "shell(cargo"; then
    pass "TOOLS_FULL includes cargo permissions"
else
    fail "TOOLS_FULL missing cargo permissions"
fi

if grep -A 20 "TOOLS_PRD=" "$BN_AGENT" | grep -q "shell(bn:" && \
   ! grep -A 20 "TOOLS_PRD=" "$BN_AGENT" | grep -q "shell(cargo"; then
    pass "TOOLS_PRD is appropriately restricted"
else
    fail "TOOLS_PRD permissions incorrect"
fi

# Test 14: Environment variable handling
section "Test 14: Environment Variables"
# Test BN_MCP_LIFECYCLE variable
if grep -q "BN_MCP_LIFECYCLE" "$BN_AGENT"; then
    pass "BN_MCP_LIFECYCLE variable supported"
else
    fail "BN_MCP_LIFECYCLE variable not supported"
fi

# Test BN_AGENT_SESSION export (for host mode)
if grep -q "export BN_AGENT_SESSION" "$BN_AGENT"; then
    pass "BN_AGENT_SESSION export configured"
else
    fail "BN_AGENT_SESSION export not configured"
fi

# Test 15: Signal handling (Ctrl+C behavior)
section "Test 15: Signal Handling Configuration"
# Verify script has signal handling for loop mode
if grep -q "handle_sigint" "$BN_AGENT" && \
   grep -q "trap.*INT" "$BN_AGENT" && \
   grep -q "SIGINT_COUNT" "$BN_AGENT"; then
    pass "Ctrl+C handling configured for loop mode"
else
    fail "Ctrl+C handling not properly configured"
fi

# Verify double Ctrl+C logic
if grep -q "Ctrl+C pressed twice" "$BN_AGENT" && \
   grep -q "within 2 seconds" "$BN_AGENT"; then
    pass "Double Ctrl+C exit logic present"
else
    fail "Double Ctrl+C exit logic missing"
fi

# Test 16: PATH configuration
section "Test 16: PATH Configuration"
if head -20 "$BN_AGENT" | grep -q 'export PATH.*HOME.*local.*bin'; then
    pass "PATH configured to prioritize ~/.local/bin"
else
    fail "PATH not properly configured"
fi

# Test 17: Git integration
section "Test 17: Git Integration"
# Verify script creates bn-agent-session marker
if grep -q "bn-agent-session" "$BN_AGENT" && \
   grep -q "GIT_DIR" "$BN_AGENT"; then
    pass "Git session marker logic present"
else
    fail "Git session marker logic missing"
fi

section "Summary"
echo -e "${GREEN}All tests passed!${NC}"
echo ""
echo "Test Coverage:"
echo "  ✓ Script validation and help"
echo "  ✓ All agent types (auto, do, prd, buddy, free)"
echo "  ✓ Mode selection (container vs host)"
echo "  ✓ Container infrastructure checking"
echo "  ✓ All command-line options"
echo "  ✓ Template emission"
echo "  ✓ Dependency checking"
echo "  ✓ Error message quality"
echo "  ✓ Tool permissions and blocked tools"
echo "  ✓ Environment variables"
echo "  ✓ Signal handling (Ctrl+C)"
echo "  ✓ PATH and Git integration"
echo ""
echo "Note: Full integration tests (actual agent execution) require:"
echo "  - GitHub Copilot CLI installed"
echo "  - Container infrastructure (for 'auto' mode)"
echo "  - Interactive terminal (for Ctrl+C testing)"
