#!/usr/bin/env bash
# shellcheck disable=SC2016  # Backticks in prompts are intentional literals
set -e

# Blocked commands - agents should not terminate each other or themselves
BLOCKED_TOOLS=(
    --deny-tool "shell(bn agent kill:*)"
    --deny-tool "shell(bn goodbye:*)"
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
    --allow-tool "shell(cargo audit)"
    --allow-tool "shell(sleep)"
    --allow-tool "shell(wait)"
    --allow-tool "shell(git add)"
    --allow-tool "shell(git commit)"
    --allow-tool "shell(git:*)"
    --allow-tool "shell(just:*)"
    --allow-tool "shell(jq:*)"
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
Usage: ./agent.sh <agent-type> [args]

Agent Types:
  auto              Pick a task from 'bn ready' and work on it immediately
  do "desc"         Work on custom task described in the argument
  prd               Find open ideas and render them into PRDs
  buddy             Ask what bn operation to perform (insert bugs/tasks/ideas)
  free              General purpose with binnacle orientation

Examples:
  ./agent.sh auto
  ./agent.sh do "find work related to gui alignment"
  ./agent.sh prd
  ./agent.sh buddy
  ./agent.sh free
EOF
    exit 1
}

# Require at least one argument
[[ $# -lt 1 ]] && usage

AGENT_TYPE="$1"
shift

case "$AGENT_TYPE" in
    auto)
        echo "Launching Auto Worker Agent"
        PROMPT='Read PRD.md and use your binnacle skill to determine the most important next action, then take it, test it, report its results, and commit it. Look for newly created tasks first. Run `bn ready` to find available tasks, pick the highest priority one, claim it with `bn task update ID --status in_progress`, and start working immediately. Remember to mark it complete when you finish.'
        copilot "${BLOCKED_TOOLS[@]}" "${TOOLS_FULL[@]}" -i "$PROMPT"
        ;;
    do)
        [[ $# -lt 1 ]] && { echo "Error: 'do' requires a description argument"; usage; }
        DESC="$1"
        echo "Launching Make Agent: $DESC"
        PROMPT="Read PRD.md and use your binnacle skill to orient yourself. Then work on the following: $DESC. Test your changes, report results, and commit when complete. Create a task or bug in binnacle if one doesn't exist for this work."
        copilot "${BLOCKED_TOOLS[@]}" "${TOOLS_FULL[@]}" -i "$PROMPT"
        ;;
    prd)
        echo "Launching PRD Writer Agent"
        PROMPT='Read PRD.md and use your binnacle skill to orient yourself. Your job is to find open ideas (tasks tagged with "idea" or in IDEAS.md) and help render them into proper PRDs. Check `bn task list --tag idea` and IDEAS.md for candidates. Pick the most promising idea and write a detailed PRD for it, then commit your work.'
        copilot "${BLOCKED_TOOLS[@]}" "${TOOLS_PRD[@]}" -i "$PROMPT"
        ;;
    buddy)
        echo "Launching Buddy Agent"
        PROMPT='You are a binnacle buddy. Your job is to help the user quickly insert bugs, tasks, and ideas into the binnacle task graph. Run `bn orient` to understand the current state. Then ask the user what they would like to add or modify in binnacle. Keep interactions quick and focused on bn operations.'
        copilot "${BLOCKED_TOOLS[@]}" "${TOOLS_BUDDY[@]}" -i "$PROMPT"
        ;;
    free)
        echo "Launching Free Agent"
        PROMPT='You have access to binnacle (bn), a task/test tracking tool for this project. Key commands: `bn orient` (get overview), `bn ready` (see available tasks), `bn task list` (all tasks), `bn task show ID` (task details), `bn blocked` (blocked tasks). Run `bn orient` to see the current project state, then ask the user what they would like you to work on.'
        copilot "${BLOCKED_TOOLS[@]}" "${TOOLS_FULL[@]}" -i "$PROMPT"
        ;;
    *)
        echo "Error: Unknown agent type '$AGENT_TYPE'"
        usage
        ;;
esac
