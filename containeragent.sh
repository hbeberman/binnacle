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
  --loop                Continuously respawn agents after exit (unattended mode)

Agent Types:
  auto                Pick a task from 'bn ready' and work on it immediately
  do "desc"           Work on custom task described in the argument
  prd                 Find open ideas and render them into PRDs
  buddy               Ask what bn operation to perform (insert bugs/tasks/ideas)
  free                General purpose with binnacle orientation

Examples:
  ./containeragent.sh auto
  ./containeragent.sh --cpus 2 --memory 4g auto
  ./containeragent.sh --loop auto                  # Continuous unattended mode
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
LOOP=""

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
        --loop)
            LOOP="true"
            shift
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

case "$AGENT_TYPE" in
    auto)
        echo "Launching Container Auto Worker Agent"
        PROMPT=$(emit_template auto-worker)
        ;;
    do)
        [[ $# -lt 1 ]] && { echo "Error: 'do' requires a description argument"; usage; }
        [[ $# -gt 1 ]] && { echo "Error: Too many arguments. Did you forget to quote the description?"; echo "  Try: ./containeragent.sh do \"$*\""; exit 1; }
        DESC="$1"
        echo "Launching Container Make Agent: $DESC"
        # Get template and substitute {description} placeholder
        PROMPT=$(emit_template do-agent | awk -v desc="$DESC" '{gsub(/{description}/, desc); print}')
        PROMPT+=$(emit_template mcp-lifecycle)
        ;;
    prd)
        echo "Launching Container PRD Writer Agent"
        PROMPT=$(emit_template prd-writer)
        PROMPT+=$(emit_template mcp-lifecycle-planner)
        ;;
    buddy)
        echo "Launching Container Buddy Agent"
        PROMPT=$(emit_template buddy)
        PROMPT+=$(emit_template mcp-lifecycle)
        ;;
    free)
        echo "Launching Container Free Agent"
        PROMPT=$(emit_template free)
        PROMPT+=$(emit_template mcp-lifecycle)
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

# Run the container (with optional looping)
run_container() {
    echo "Running: ${CMD[*]}"
    "${CMD[@]}"
}

if [[ -n "$LOOP" ]]; then
    echo "Loop mode enabled - will continuously respawn agents"
    ITERATION=1
    while true; do
        echo ""
        echo "=== Starting agent iteration $ITERATION ==="
        echo ""
        # Disable errexit for the container run so we continue looping on failure
        set +e
        run_container
        EXIT_CODE=$?
        set -e
        echo ""
        echo "=== Agent iteration $ITERATION exited with code $EXIT_CODE ==="
        ITERATION=$((ITERATION + 1))
        # Brief pause before respawning to allow for Ctrl+C
        echo "Respawning in 3 seconds... (Ctrl+C to stop)"
        sleep 3
    done
else
    exec "${CMD[@]}"
fi
