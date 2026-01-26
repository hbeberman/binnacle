#!/usr/bin/env bash
# shellcheck disable=SC2016  # Backticks in prompts are intentional literals
set -e

# Blocked commands - agents should not terminate each other
# Note: bn goodbye is allowed - agents SHOULD terminate themselves gracefully when done
BLOCKED_TOOLS=(
    --deny-tool "shell(bn agent kill:*)"
    # Block MCP lifecycle tools to force shell usage for proper agent termination
    # See PRD_MCP_SIMPLIFICATION.md Part 3: Agent Lifecycle for MCP
    --deny-tool "binnacle(binnacle-orient)"
    --deny-tool "binnacle(binnacle-goodbye)"
)

# MCP lifecycle guidance - appended to prompts for agents launched via this script
# This explains why orient/goodbye must use shell instead of MCP tools
MCP_LIFECYCLE_BLURB='

IMPORTANT - Binnacle MCP Lifecycle:
You have access to binnacle MCP tools, but orient and goodbye MUST use shell commands:
- Use `bn orient --type "agent type"` via shell (NOT binnacle-orient MCP tool)
- Use `bn goodbye "summary"` via shell (NOT binnacle-goodbye MCP tool)
- Other bn commands should use the MCP tools for preference.

To link your MCP session to your shell agent registration:
1. Run `bn orient --type "agent type"` via shell - note the `agent_id` (e.g., "bna-1234") in the output
2. Call binnacle-set_agent MCP tool with path="/your/repo" and session_id="<agent_id from step 1>"
3. Now all MCP bn_run calls will be attributed to your agent session

This is because MCP tools cannot terminate your process - only shell goodbye can do that.
The MCP orient/goodbye tools are blocked for this reason.

FALLBACK - If MCP Tools Are Unavailable:
If binnacle MCP tools are not available you may use the `bn` shell commands directly as a last resort fallback (excepting orient and goodbye).'

# Tool permission sets (using arrays to properly handle arguments with spaces)
TOOLS_FULL=(
    --allow-tool "write"
    --allow-tool "shell(bn:*)"
    --allow-tool "shell(./target/release/bn)"
    --allow-tool "shell(./target/debug/bn)"
    --allow-tool "shell(cargo run)"
    --allow-tool "shell(cargo fmt)"
    --allow-tool "shell(cargo clippy)"
    --allow-tool "shell(cargo test)"
    --allow-tool "shell(cargo build)"
    --allow-tool "shell(cargo check)"
    --allow-tool "shell(cargo audit)"
    --allow-tool "shell(rustc:*)"
    --allow-tool "shell(sleep)"
    --allow-tool "shell(wait)"
    --allow-tool "shell(git add)"
    --allow-tool "shell(git commit)"
    --allow-tool "shell(git:*)"
    --allow-tool "shell(just:*)"
    --allow-tool "shell(jq:*)"
    --allow-tool "shell(sed:*)"
    --allow-tool "shell(cp:*)"
    --allow-tool "shell(lsof:*)"
    --allow-tool "shell(rm:*)"
    --allow-tool "shell(mkdir:*)"
    --allow-tool "shell(xargs:*)"
    --allow-tool "shell(find:*)"
    --allow-tool "shell(printf:*)"
    --allow-tool "shell(curl:*)"
    --allow-tool "shell(awk:*)"
    --allow-tool "shell(pgrep:*)"
    --allow-tool "shell(node:*)"
    --allow-tool "binnacle"
)

TOOLS_PRD=(
    --allow-tool "write"
    --allow-tool "shell(bn:*)"
    --allow-tool "shell(./target/release/bn)"
    --allow-tool "shell(./target/debug/bn)"
    --allow-tool "shell(git add)"
    --allow-tool "shell(git commit)"
    --allow-tool "shell(git:*)"
    --allow-tool "shell(jq:*)"
    --allow-tool "binnacle"
)

TOOLS_BUDDY=(
    --allow-tool "shell(bn:*)"
    --allow-tool "shell(./target/release/bn)"
    --allow-tool "shell(./target/debug/bn)"
    --allow-tool "shell(git add)"
    --allow-tool "shell(git commit)"
    --allow-tool "shell(git:*)"
    --allow-tool "shell(jq:*)"
    --allow-tool "binnacle"
)

usage() {
    cat << 'EOF'
Usage: ./agent.sh [--loop] <agent-type> [args]

Global Options:
  --loop              Restart the agent when it exits (works with any agent type)

Agent Types:
  auto                Pick a task from 'bn ready' and work on it immediately
  do "desc"           Work on custom task described in the argument
  prd                 Find open ideas and render them into PRDs
  buddy               Ask what bn operation to perform (insert bugs/tasks/ideas)
  free                General purpose with binnacle orientation

Examples:
  ./agent.sh auto
  ./agent.sh --loop auto
  ./agent.sh do "find work related to gui alignment"
  ./agent.sh --loop do "fix the login bug"
  ./agent.sh prd
  ./agent.sh buddy
  ./agent.sh free
EOF
    exit 1
}

# Require at least one argument
[[ $# -lt 1 ]] && usage

# Check for global --loop flag
LOOP_MODE=false
if [[ "$1" == "--loop" ]]; then
    LOOP_MODE=true
    shift
fi

[[ $# -lt 1 ]] && usage

AGENT_TYPE="$1"
shift

case "$AGENT_TYPE" in
    auto)
        echo "Launching Auto Worker Agent"
        PROMPT=$(bn system emit auto-worker -H)
        PROMPT+="$MCP_LIFECYCLE_BLURB"
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    do)
        [[ $# -lt 1 ]] && { echo "Error: 'do' requires a description argument"; usage; }
        [[ $# -gt 1 ]] && { echo "Error: Too many arguments. Did you forget to quote the description?"; echo "  Try: ./agent.sh do \"$*\""; exit 1; }
        DESC="$1"
        echo "Launching Make Agent: $DESC"
        # Get template and substitute {description} placeholder
        # Use awk with -v to safely handle arbitrary strings (special chars, slashes, etc.)
        PROMPT=$(bn system emit do-agent -H | awk -v desc="$DESC" '{gsub(/{description}/, desc); print}')
        PROMPT+="$MCP_LIFECYCLE_BLURB"
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    prd)
        echo "Launching PRD Writer Agent"
        PROMPT=$(bn system emit prd-writer -H)
        TOOLS=("${TOOLS_PRD[@]}")
        ;;
    buddy)
        echo "Launching Buddy Agent"
        PROMPT=$(bn system emit buddy -H)
        TOOLS=("${TOOLS_BUDDY[@]}")
        ;;
    free)
        echo "Launching Free Agent"
        PROMPT=$(bn system emit free -H)
        PROMPT+="$MCP_LIFECYCLE_BLURB"
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    *)
        echo "Error: Unknown agent type '$AGENT_TYPE'"
        usage
        ;;
esac

# Export BN_AGENT_SESSION so child processes (including git hooks) know they're running under an agent
export BN_AGENT_SESSION=1

# Create a marker file in .git directory so commit hooks can detect agent activity
# This works even when the copilot CLI doesn't inherit environment variables
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null) || GIT_DIR=""
BN_AGENT_MARKER=""
if [[ -n "$GIT_DIR" ]]; then
    BN_AGENT_MARKER="$GIT_DIR/bn-agent-session"
    echo "$$" > "$BN_AGENT_MARKER"

    # Clean up marker file on exit
    cleanup_marker() {
        [[ -n "$BN_AGENT_MARKER" ]] && rm -f "$BN_AGENT_MARKER"
    }
    trap cleanup_marker EXIT
fi

# Run the agent (with optional loop)
# Common copilot flags for all agent types
COPILOT_FLAGS=(
    --allow-all-urls  # Allow network requests without prompting
)

if [[ "$LOOP_MODE" == "true" ]]; then
    echo "Loop mode enabled - agent will restart on exit"
    # Track consecutive Ctrl+C presses for clean exit
    SIGINT_COUNT=0
    LAST_SIGINT=0

    handle_sigint() {
        local now
        now=$(date +%s)
        # If Ctrl+C pressed twice within 2 seconds, exit
        if [[ $((now - LAST_SIGINT)) -le 2 ]]; then
            SIGINT_COUNT=$((SIGINT_COUNT + 1))
        else
            SIGINT_COUNT=1
        fi
        LAST_SIGINT=$now

        if [[ $SIGINT_COUNT -ge 2 ]]; then
            echo ""
            echo "Ctrl+C pressed twice - exiting loop mode"
            exit 0
        fi
        echo ""
        echo "(Press Ctrl+C again within 2 seconds to exit loop mode)"
    }

    trap handle_sigint INT

    while true; do
        # Reset SIGINT count at start of each iteration
        SIGINT_COUNT=0
        copilot "${COPILOT_FLAGS[@]}" "${BLOCKED_TOOLS[@]}" "${TOOLS[@]}" -i "$PROMPT" || true
        echo ""
        echo "Agent exited. Restarting in 3 seconds... (Ctrl+C twice to stop)"
        sleep 3
    done
else
    copilot "${COPILOT_FLAGS[@]}" "${BLOCKED_TOOLS[@]}" "${TOOLS[@]}" -i "$PROMPT"
fi
