#!/usr/bin/env bash
# shellcheck disable=SC2016  # Backticks in prompts are intentional literals
# Container Agent Launcher
# Mirrors agent.sh but runs agents inside binnacle containers using `bn container run`
set -e

usage() {
    cat << 'EOF'
Usage: ./containeragent.sh [OPTIONS] <agent-type> [args]

Container Options:
  --cpus LIMIT          CPU limit (e.g., 1.5 for 1.5 CPUs)
  --memory LIMIT        Memory limit (e.g., "512m", "1g")
  --merge-target BRANCH Branch to merge into on exit (default: main)
  --no-merge            Disable auto-merge on exit
  --name NAME           Container name (auto-generated if not provided)

Agent Types:
  auto                Pick a task from 'bn ready' and work on it immediately
  do "desc"           Work on custom task described in the argument
  prd                 Find open ideas and render them into PRDs
  buddy               Ask what bn operation to perform (insert bugs/tasks/ideas)
  free                General purpose with binnacle orientation

Examples:
  ./containeragent.sh auto
  ./containeragent.sh --cpus 2 --memory 4g auto
  ./containeragent.sh do "find work related to gui alignment"
  ./containeragent.sh --no-merge prd
  ./containeragent.sh buddy
  ./containeragent.sh free
EOF
    exit 1
}

# Require at least one argument
[[ $# -lt 1 ]] && usage

# Parse container options
CPUS=""
MEMORY=""
MERGE_TARGET="main"
NO_MERGE=""
NAME=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --cpus)
            CPUS="$2"
            shift 2
            ;;
        --memory)
            MEMORY="$2"
            shift 2
            ;;
        --merge-target)
            MERGE_TARGET="$2"
            shift 2
            ;;
        --no-merge)
            NO_MERGE="true"
            shift
            ;;
        --name)
            NAME="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            # First non-option argument is the agent type
            break
            ;;
    esac
done

[[ $# -lt 1 ]] && usage

AGENT_TYPE="$1"
shift

case "$AGENT_TYPE" in
    auto)
        echo "Launching Container Auto Worker Agent"
        PROMPT='Run `bn orient --type worker` to get oriented with the project. Read PRD.md and use your binnacle skill to determine the most important next action, then take it, test it, report its results, and commit it. Run `bn ready` to find available tasks and bugs. IMPORTANT: Prioritize queued items first (items with "queued": true in the JSON output) - these have been explicitly marked as high priority by an operator. Among queued items, pick by priority (lower number = higher priority). If no queued items exist, pick the highest priority non-queued item. Claim your chosen item with `bn task update ID --status in_progress` or `bn bug update ID --status in_progress`, and start working immediately. Remember to mark it complete when you finish. Run `bn goodbye "summary of what was accomplished"` to gracefully terminate your agent session when all work is done.'
        ;;
    do)
        [[ $# -lt 1 ]] && { echo "Error: 'do' requires a description argument"; usage; }
        [[ $# -gt 1 ]] && { echo "Error: Too many arguments. Did you forget to quote the description?"; echo "  Try: ./containeragent.sh do \"$*\""; exit 1; }
        DESC="$1"
        echo "Launching Container Make Agent: $DESC"
        PROMPT="Run \`bn orient --type worker\` to get oriented with the project. Read PRD.md. Then work on the following: $DESC. Test your changes, report results, and commit when complete. Create a task or bug in binnacle if one doesn't exist for this work. Run \`bn goodbye \"summary of what was accomplished\"\` to gracefully terminate your agent session when all work is done."
        ;;
    prd)
        echo "Launching Container PRD Writer Agent"
        PROMPT='Run `bn orient --type planner` to get oriented with the project. Read PRD.md. Your job is to help render ideas into proper PRDs. First, ask the user: "Do you have a specific idea or topic in mind, or would you like me to pick one from the open ideas?" 

CRITICAL: Before writing ANY PRD, ALWAYS run `bn idea list -H` to search for existing ideas related to the topic. This ensures you build upon existing thoughts and do not duplicate work. If you find related ideas:
1. Reference them in the PRD (e.g., "Related ideas: bn-xxxx, bn-yyyy")
2. Incorporate their insights into the PRD content
3. Consider whether the PRD should supersede/combine multiple related ideas

If the user provides a topic, search ideas for that topic first, then work on it. If no topic provided, check `bn idea list` for candidates and pick the most promising one. Then STOP and ask clarifying questions before writing the PRD. Ask about: scope boundaries (what is in/out), target users, success criteria, implementation constraints, dependencies on other work, and priority relative to other features.

IMPORTANT - Store PRDs as doc nodes, not files:
After gathering requirements and writing the PRD content, use `bn doc create` to store it in the task graph:
  bn doc create <related-entity-id> --type prd --title "PRD: Feature Name" --content "...prd content..."
Or to read from a file:
  bn doc create <related-entity-id> --type prd --title "PRD: Feature Name" --file /tmp/prd.md
The <related-entity-id> should be the idea being promoted, or a task/milestone this PRD relates to.

Do NOT save PRDs to prds/ directory - use doc nodes so PRDs are tracked, linked, and versioned in the graph.
Do NOT run `bn goodbye` - planner agents produce artifacts but do not run long-lived sessions.'
        ;;
    buddy)
        echo "Launching Container Buddy Agent"
        PROMPT='You are a binnacle buddy. Your job is to help the user quickly insert bugs, tasks, and ideas into the binnacle task graph. Run `bn orient --type buddy` to understand the current state. Then ask the user what they would like to add or modify in binnacle. Keep interactions quick and focused on bn operations.

IMPORTANT - Use the correct entity type and ALWAYS include a short name (-s):
- `bn idea create -s "short" "Full title"` for rough thoughts, exploratory concepts, or "what if" suggestions that need discussion/refinement before becoming actionable work
- `bn task create -s "short" "Full title"` for specific, actionable work items that are ready to be implemented
- `bn bug create -s "short" "Full title"` for defects, problems, or issues that need fixing

Short names appear in the GUI and make entities much easier to scan. Keep them to 2-4 words.

When the user says "idea", "thought", "what if", "maybe we could", "explore", or similar exploratory language, ALWAYS use `bn idea create`. Ideas are low-stakes and can be promoted to tasks later.

TASK DECOMPOSITION - Break down tasks into subtasks:
When creating a task, look for opportunities to decompose it into 2-4 smaller, independent subtasks. This helps agents work on focused pieces. To decompose:
1. Create the parent task first: `bn task create "Parent task title" -s "short name" -d "description"`
2. Create each subtask: `bn task create "Subtask title" -s "subtask short" -d "description"`
3. Link subtasks to parent: `bn link add <subtask-id> <parent-id> -t child_of`

Good candidates for decomposition:
- Tasks with multiple distinct steps (e.g., "add X and test Y" → separate implementation and testing tasks)
- Tasks touching multiple components (e.g., "update CLI and GUI" → separate CLI and GUI tasks)
- Tasks with setup requirements (e.g., "configure X then implement Y" → separate configuration and implementation)

Do NOT decompose:
- Simple, single-action tasks (e.g., "fix typo in README")
- Tasks that are already focused and atomic
- Ideas (decomposition happens when ideas are promoted to tasks)

CRITICAL - Always check the graph for latest state:
When answering questions about bugs, tasks, or ideas (even ones you created earlier in this session), ALWAYS run `bn show <id>` to check the current state. Never assume an entity is still open just because you created it - another agent or human may have closed it. The graph is the source of truth, not your session memory.

Run `bn goodbye "session complete"` to gracefully terminate your agent session when the user is done.'
        ;;
    free)
        echo "Launching Container Free Agent"
        PROMPT='You have access to binnacle (bn), a task/test tracking tool for this project. Key commands: `bn orient --type worker` (get overview), `bn ready` (see available tasks), `bn task list` (all tasks), `bn show ID` (show any entity - works with bn-/bnt-/bnq- prefixes), `bn blocked` (blocked tasks). Run `bn orient --type worker` to see the current project state, then ask the user what they would like you to work on. Run `bn goodbye "summary of what was accomplished"` to gracefully terminate your agent session when all work is done.'
        ;;
    *)
        echo "Error: Unknown agent type '$AGENT_TYPE'"
        usage
        ;;
esac

# Build the bn container run command
CMD=(bn container run .)

# Add container options
[[ -n "$CPUS" ]] && CMD+=(--cpus "$CPUS")
[[ -n "$MEMORY" ]] && CMD+=(--memory "$MEMORY")
[[ -n "$NAME" ]] && CMD+=(--name "$NAME")
CMD+=(--merge-target "$MERGE_TARGET")
[[ -n "$NO_MERGE" ]] && CMD+=(--no-merge)

# Add the prompt
CMD+=(--prompt "$PROMPT")

# Run the container
echo "Running: ${CMD[*]}"
exec "${CMD[@]}"
