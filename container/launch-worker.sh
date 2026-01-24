#!/bin/bash
# Binnacle Container Worker Launcher
# Usage: ./launch-worker.sh /path/to/worktree [agent-type]
#
# This script sets up the correct environment and launches a containerized
# AI agent with access to the binnacle task graph.

set -e

# Parse arguments
WORKTREE_PATH="${1:?Usage: $0 /path/to/worktree [agent-type]}"
AGENT_TYPE="${2:-worker}"

# Resolve absolute path
WORKTREE_PATH="$(cd "$WORKTREE_PATH" && pwd)"

# Validate worktree is a git repository
if [ ! -d "$WORKTREE_PATH/.git" ] && [ ! -f "$WORKTREE_PATH/.git" ]; then
    echo "❌ Error: $WORKTREE_PATH is not a git repository or worktree"
    exit 1
fi

# Get repo root and compute hash for binnacle data path
REPO_ROOT=$(cd "$WORKTREE_PATH" && git rev-parse --show-toplevel)
REPO_HASH=$(echo -n "$REPO_ROOT" | sha256sum | cut -c1-12)
BINNACLE_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/binnacle/$REPO_HASH"

# Ensure binnacle data directory exists
mkdir -p "$BINNACLE_DATA"

echo "🚀 Launching binnacle worker..."
echo "   Worktree:      $WORKTREE_PATH"
echo "   Agent type:    $AGENT_TYPE"
echo "   Binnacle data: $BINNACLE_DATA"
echo ""

# Check for auth token
if [ -z "$COPILOT_GITHUB_TOKEN" ] && [ -z "$GH_TOKEN" ] && [ -z "$GITHUB_TOKEN" ]; then
    echo "⚠️  Warning: No COPILOT_GITHUB_TOKEN, GH_TOKEN, or GITHUB_TOKEN set"
    echo "   Set one of these environment variables for AI agent auth"
    echo ""
fi

# Get the directory containing this script
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Export variables for docker compose
export WORKTREE_PATH
export BINNACLE_DATA_PATH="$BINNACLE_DATA"
export BN_AGENT_TYPE="$AGENT_TYPE"

# Launch container
cd "$SCRIPT_DIR"
docker compose up --build binnacle-worker
