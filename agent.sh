#!/usr/bin/env bash
# shellcheck disable=SC2016  # Backticks in prompts are intentional literals
set -e

# Ensure ~/.local/bin is first in PATH to use latest 'just install' binary
export PATH="$HOME/.local/bin:$PATH"

# Blocked commands - agents should not terminate each other
# Note: bn goodbye is allowed - agents SHOULD terminate themselves gracefully when done
BLOCKED_TOOLS=(
    --deny-tool "shell(bn agent kill:*)"
    # Block MCP lifecycle tools to force shell usage for proper agent termination
    # See PRD_MCP_SIMPLIFICATION.md Part 3: Agent Lifecycle for MCP
    --deny-tool "binnacle(binnacle-orient)"
    --deny-tool "binnacle(binnacle-goodbye)"
)

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

TOOLS_ASK=(
    # bn read-only commands
    --allow-tool "shell(bn orient)"
    --allow-tool "shell(bn ready)"
    --allow-tool "shell(bn blocked)"
    --allow-tool "shell(bn task list)"
    --allow-tool "shell(bn task show)"
    --allow-tool "shell(bn test list)"
    --allow-tool "shell(bn test show)"
    --allow-tool "shell(bn bug list)"
    --allow-tool "shell(bn bug show)"
    --allow-tool "shell(bn idea list)"
    --allow-tool "shell(bn idea show)"
    --allow-tool "shell(bn queue show)"
    --allow-tool "shell(bn link list)"
    --allow-tool "shell(bn log)"
    --allow-tool "shell(bn show)"
    --allow-tool "shell(bn goodbye)"
    # Git read-only
    --allow-tool "shell(git log)"
    --allow-tool "shell(git show)"
    --allow-tool "shell(git diff)"
    --allow-tool "shell(git blame)"
    --allow-tool "shell(git status)"
    --allow-tool "shell(git branch)"
    # File exploration (read-only via copilot's view/grep/glob)
    --allow-tool "binnacle"
)

usage() {
    cat << 'EOF'
Usage: ./agent.sh [--loop] <agent-type> [args]

Global Options:
  --loop              Restart the agent when it exits (works with any agent type)

Environment Variables:
  BN_MCP_LIFECYCLE    Control MCP lifecycle prompts (default: true)
                      Set to "false" to disable MCP orient/goodbye guidance

Agent Types:
  auto                Pick a task from 'bn ready' and work on it immediately
  do "desc"           Work on custom task described in the argument
  prd                 Find open ideas and render them into PRDs
  buddy               Ask what bn operation to perform (insert bugs/tasks/ideas)
  ask                 Interactive Q&A for exploring the codebase (read-only)
  free                General purpose with binnacle orientation

Examples:
  ./agent.sh auto
  ./agent.sh --loop auto
  ./agent.sh do "find work related to gui alignment"
  ./agent.sh --loop do "fix the login bug"
  ./agent.sh prd
  ./agent.sh buddy
  ./agent.sh ask
  ./agent.sh free
  BN_MCP_LIFECYCLE=false ./agent.sh auto
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

# Helper function to safely emit templates with error handling
emit_template() {
    local template="$1"
    local output
    if ! output=$(bn system emit "$template" -H 2>&1); then
        echo "❌ Failed to emit template: $template" >&2
        echo "   Error: $output" >&2
        exit 1
    fi
    echo "$output"
}

# Check if MCP lifecycle prompts should be added (default: true)
BN_MCP_LIFECYCLE="${BN_MCP_LIFECYCLE:-true}"

case "$AGENT_TYPE" in
    auto)
        echo "Launching Auto Worker Agent"
        PROMPT=$(emit_template auto-worker)
        [[ "$BN_MCP_LIFECYCLE" == "true" ]] && PROMPT+=$(emit_template mcp-lifecycle)
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    do)
        [[ $# -lt 1 ]] && { echo "Error: 'do' requires a description argument"; usage; }
        [[ $# -gt 1 ]] && { echo "Error: Too many arguments. Did you forget to quote the description?"; echo "  Try: ./agent.sh do \"$*\""; exit 1; }
        DESC="$1"
        echo "Launching Make Agent: $DESC"
        # Get template and substitute {description} placeholder
        # Use awk with -v to safely handle arbitrary strings (special chars, slashes, etc.)
        PROMPT=$(emit_template do-agent | awk -v desc="$DESC" '{gsub(/{description}/, desc); print}')
        [[ "$BN_MCP_LIFECYCLE" == "true" ]] && PROMPT+=$(emit_template mcp-lifecycle)
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    prd)
        echo "Launching PRD Writer Agent"
        PROMPT=$(emit_template prd-writer)
        [[ "$BN_MCP_LIFECYCLE" == "true" ]] && PROMPT+=$(emit_template mcp-lifecycle-planner)
        TOOLS=("${TOOLS_PRD[@]}")
        ;;
    buddy)
        echo "Launching Buddy Agent"
        PROMPT=$(emit_template buddy)
        [[ "$BN_MCP_LIFECYCLE" == "true" ]] && PROMPT+=$(emit_template mcp-lifecycle)
        TOOLS=("${TOOLS_BUDDY[@]}")
        ;;
    ask)
        echo "Launching Ask Agent"
        PROMPT=$(emit_template ask-agent)
        [[ "$BN_MCP_LIFECYCLE" == "true" ]] && PROMPT+=$(emit_template mcp-lifecycle)
        TOOLS=("${TOOLS_ASK[@]}")
        ;;
    free)
        echo "Launching Free Agent"
        PROMPT=$(emit_template free)
        [[ "$BN_MCP_LIFECYCLE" == "true" ]] && PROMPT+=$(emit_template mcp-lifecycle)
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

# Resolve copilot binary path using bn system copilot path
COPILOT_JSON=$(bn system copilot path 2>&1) || {
    echo "❌ Failed to resolve copilot binary path" >&2
    echo "   Error: $COPILOT_JSON" >&2
    exit 1
}

COPILOT_PATH=$(echo "$COPILOT_JSON" | jq -r '.path')
COPILOT_EXISTS=$(echo "$COPILOT_JSON" | jq -r '.exists')
COPILOT_VERSION=$(echo "$COPILOT_JSON" | jq -r '.version')
COPILOT_SOURCE=$(echo "$COPILOT_JSON" | jq -r '.source')

if [[ "$COPILOT_EXISTS" != "true" ]]; then
    echo "❌ Copilot $COPILOT_VERSION ($COPILOT_SOURCE) not found at: $COPILOT_PATH" >&2
    echo "   Run 'bn system copilot install' to install it" >&2
    exit 1
fi

# Run the agent (with optional loop)
# Common copilot flags for all agent types
COPILOT_FLAGS=(
    --allow-all-urls  # Allow network requests without prompting
    --no-auto-update  # Prevent automatic updates during execution
    --staff           # Enable staff mode for LSP support
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
        "$COPILOT_PATH" "${COPILOT_FLAGS[@]}" "${BLOCKED_TOOLS[@]}" "${TOOLS[@]}" -i "$PROMPT" || true
        echo ""
        echo "Agent exited. Restarting in 3 seconds... (Ctrl+C twice to stop)"
        sleep 3
    done
else
    "$COPILOT_PATH" "${COPILOT_FLAGS[@]}" "${BLOCKED_TOOLS[@]}" "${TOOLS[@]}" -i "$PROMPT"
fi
