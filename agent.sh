#!/usr/bin/env bash
# shellcheck disable=SC2016  # Backticks in prompts are intentional literals
set -e

# Blocked commands - agents should not terminate each other
# Note: bn goodbye is allowed - agents SHOULD terminate themselves gracefully when done
BLOCKED_TOOLS=(
    --deny-tool "shell(bn agent kill:*)"
)

# Tool permission sets (using arrays to properly handle arguments with spaces)
TOOLS_FULL=(
    --allow-tool "write"
    --allow-tool "shell(bn:*)"
    --allow-tool "shell(./target/release/bn)"
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
)

TOOLS_PRD=(
    --allow-tool "write"
    --allow-tool "shell(bn:*)"
    --allow-tool "shell(./target/release/bn)"
    --allow-tool "shell(git add)"
    --allow-tool "shell(git commit)"
    --allow-tool "shell(git:*)"
    --allow-tool "shell(jq:*)"
)

TOOLS_BUDDY=(
    --allow-tool "shell(bn:*)"
    --allow-tool "shell(./target/release/bn)"
    --allow-tool "shell(git add)"
    --allow-tool "shell(git commit)"
    --allow-tool "shell(git:*)"
    --allow-tool "shell(jq:*)"
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
        PROMPT='Run `bn orient --type worker` to get oriented with the project. Read PRD.md and use your binnacle skill to determine the most important next action, then take it, test it, report its results, and commit it. Run `bn ready` to find available tasks and bugs. IMPORTANT: Prioritize queued items first (items with "queued": true in the JSON output) - these have been explicitly marked as high priority by an operator. Among queued items, pick by priority (lower number = higher priority). If no queued items exist, pick the highest priority non-queued item. Claim your chosen item with `bn task update ID --status in_progress` or `bn bug update ID --status in_progress`, and start working immediately. Remember to mark it complete when you finish. Run `bn goodbye "summary of what was accomplished"` to gracefully terminate your agent session when all work is done.'
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    do)
        [[ $# -lt 1 ]] && { echo "Error: 'do' requires a description argument"; usage; }
        [[ $# -gt 1 ]] && { echo "Error: Too many arguments. Did you forget to quote the description?"; echo "  Try: ./agent.sh do \"$*\""; exit 1; }
        DESC="$1"
        echo "Launching Make Agent: $DESC"
        PROMPT="Run \`bn orient --type worker\` to get oriented with the project. Read PRD.md. Then work on the following: $DESC. Test your changes, report results, and commit when complete. Create a task or bug in binnacle if one doesn't exist for this work. Run \`bn goodbye \"summary of what was accomplished\"\` to gracefully terminate your agent session when all work is done."
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    prd)
        echo "Launching PRD Writer Agent"
        PROMPT='Run `bn orient --type planner` to get oriented with the project. Read PRD.md. Your job is to help render ideas into proper PRDs. First, ask the user: "Do you have a specific idea or topic in mind, or would you like me to pick one from the open ideas?" 

CRITICAL: Before writing ANY PRD, ALWAYS run `bn idea list -H` to search for existing ideas related to the topic. This ensures you build upon existing thoughts and do not duplicate work. If you find related ideas:
1. Reference them in the PRD (e.g., "Related ideas: bn-xxxx, bn-yyyy")
2. Incorporate their insights into the PRD content
3. Consider whether the PRD should supersede/combine multiple related ideas

If the user provides a topic, search ideas for that topic first, then work on it. If no topic provided, check `bn idea list` for candidates and pick the most promising one. Then STOP and ask clarifying questions before writing the PRD. Ask about: scope boundaries (what is in/out), target users, success criteria, implementation constraints, dependencies on other work, and priority relative to other features. Only after getting answers should you write the PRD. Save PRDs to prds/ directory. Do NOT run `bn goodbye` - planner agents produce artifacts but do not run long-lived sessions.'
        TOOLS=("${TOOLS_PRD[@]}")
        ;;
    buddy)
        echo "Launching Buddy Agent"
        PROMPT='You are a binnacle buddy. Your job is to help the user quickly insert bugs, tasks, and ideas into the binnacle task graph. Run `bn orient --type buddy` to understand the current state. Then ask the user what they would like to add or modify in binnacle. Keep interactions quick and focused on bn operations.

IMPORTANT - Use the correct entity type:
- `bn idea create "..."` for rough thoughts, exploratory concepts, or "what if" suggestions that need discussion/refinement before becoming actionable work
- `bn task create "..."` for specific, actionable work items that are ready to be implemented
- `bn bug create "..."` for defects, problems, or issues that need fixing

When the user says "idea", "thought", "what if", "maybe we could", "explore", or similar exploratory language, ALWAYS use `bn idea create`. Ideas are low-stakes and can be promoted to tasks later.

CRITICAL - Always check the graph for latest state:
When answering questions about bugs, tasks, or ideas (even ones you created earlier in this session), ALWAYS run `bn show <id>` to check the current state. Never assume an entity is still open just because you created it - another agent or human may have closed it. The graph is the source of truth, not your session memory.

Run `bn goodbye "session complete"` to gracefully terminate your agent session when the user is done.'
        TOOLS=("${TOOLS_BUDDY[@]}")
        ;;
    free)
        echo "Launching Free Agent"
        PROMPT='You have access to binnacle (bn), a task/test tracking tool for this project. Key commands: `bn orient --type worker` (get overview), `bn ready` (see available tasks), `bn task list` (all tasks), `bn show ID` (show any entity - works with bn-/bnt-/bnq- prefixes), `bn blocked` (blocked tasks). Run `bn orient --type worker` to see the current project state, then ask the user what they would like you to work on. Run `bn goodbye "summary of what was accomplished"` to gracefully terminate your agent session when all work is done.'
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
        copilot "${BLOCKED_TOOLS[@]}" "${TOOLS[@]}" -i "$PROMPT" || true
        echo ""
        echo "Agent exited. Restarting in 3 seconds... (Ctrl+C twice to stop)"
        sleep 3
    done
else
    copilot "${BLOCKED_TOOLS[@]}" "${TOOLS[@]}" -i "$PROMPT"
fi
