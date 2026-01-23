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
        PROMPT='Read PRD.md and use your binnacle skill to determine the most important next action, then take it, test it, report its results, and commit it. Look for newly created tasks and bugs first. Run `bn ready` to find available tasks and bugs, pick the highest priority one, claim it with `bn task update ID --status in_progress` or `bn bug update ID --status in_progress`, and start working immediately. Remember to mark it complete when you finish. Run `bn goodbye "summary of what was accomplished"` to gracefully terminate your agent session when all work is done.'
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    do)
        [[ $# -lt 1 ]] && { echo "Error: 'do' requires a description argument"; usage; }
        [[ $# -gt 1 ]] && { echo "Error: Too many arguments. Did you forget to quote the description?"; echo "  Try: ./agent.sh do \"$*\""; exit 1; }
        DESC="$1"
        echo "Launching Make Agent: $DESC"
        PROMPT="Read PRD.md and use your binnacle skill to orient yourself. Then work on the following: $DESC. Test your changes, report results, and commit when complete. Create a task or bug in binnacle if one doesn't exist for this work. Run \`bn goodbye \"summary of what was accomplished\"\` to gracefully terminate your agent session when all work is done."
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    prd)
        echo "Launching PRD Writer Agent"
        PROMPT='Read PRD.md and use your binnacle skill to orient yourself. Your job is to help render ideas into proper PRDs. First, ask the user: "Do you have a specific idea or topic in mind, or would you like me to pick one from the open ideas?" If the user provides a topic, work on that. Otherwise, check `bn idea list` for candidates and pick the most promising one. Then STOP and ask clarifying questions before writing the PRD. Ask about: scope boundaries (what is in/out), target users, success criteria, implementation constraints, dependencies on other work, and priority relative to other features. Only after getting answers should you write the PRD. Save PRDs to prds/ directory. Run `bn goodbye "summary of PRDs created"` to gracefully terminate your agent session when all work is done.'
        TOOLS=("${TOOLS_PRD[@]}")
        ;;
    buddy)
        echo "Launching Buddy Agent"
        PROMPT='You are a binnacle buddy. Your job is to help the user quickly insert bugs, tasks, and ideas into the binnacle task graph. Run `bn orient` to understand the current state. Then ask the user what they would like to add or modify in binnacle. Keep interactions quick and focused on bn operations. Run `bn goodbye "session complete"` to gracefully terminate your agent session when the user is done.'
        TOOLS=("${TOOLS_BUDDY[@]}")
        ;;
    free)
        echo "Launching Free Agent"
        PROMPT='You have access to binnacle (bn), a task/test tracking tool for this project. Key commands: `bn orient` (get overview), `bn ready` (see available tasks), `bn task list` (all tasks), `bn task show ID` (task details), `bn blocked` (blocked tasks). Run `bn orient` to see the current project state, then ask the user what they would like you to work on. Run `bn goodbye "summary of what was accomplished"` to gracefully terminate your agent session when all work is done.'
        TOOLS=("${TOOLS_FULL[@]}")
        ;;
    *)
        echo "Error: Unknown agent type '$AGENT_TYPE'"
        usage
        ;;
esac

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
