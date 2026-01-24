#!/bin/bash
# Binnacle Container Worker Entrypoint
# This script initializes the agent environment and runs the AI agent

set -e

# Configuration with defaults
BN_AGENT_TYPE="${BN_AGENT_TYPE:-worker}"
BN_MERGE_TARGET="${BN_MERGE_TARGET:-main}"
BN_INITIAL_PROMPT="${BN_INITIAL_PROMPT:-Run bn ready to see available tasks, pick one, and complete it. Call bn goodbye when done.}"
BN_AUTO_MERGE="${BN_AUTO_MERGE:-true}"

cd /workspace

# Ensure git is configured for commits
if [ -z "$(git config --get user.email)" ]; then
    git config user.email "binnacle-worker@container.local"
    git config user.name "Binnacle Worker"
fi

# Orient the agent
echo "🧭 Orienting agent (type: $BN_AGENT_TYPE)..."
bn orient --type "$BN_AGENT_TYPE" -H

# Get current branch for merge later
WORK_BRANCH=$(git rev-parse --abbrev-ref HEAD)
echo "📍 Working on branch: $WORK_BRANCH"

# Check for shell mode (debugging)
if [ "$1" = "shell" ] || [ "$1" = "bash" ]; then
    echo "🐚 Starting interactive shell..."
    exec /bin/bash
fi

# Ensure bun is in PATH
export BUN_INSTALL="${BUN_INSTALL:-/root/.bun}"
export PATH="$BUN_INSTALL/bin:$PATH"

# Run the AI agent
echo "🤖 Starting AI agent..."
if command -v copilot &> /dev/null; then
    copilot --allow-all-tools "$BN_INITIAL_PROMPT"
    AGENT_EXIT=$?
elif command -v claude &> /dev/null; then
    claude "$BN_INITIAL_PROMPT"
    AGENT_EXIT=$?
else
    echo "❌ No AI agent found (copilot or claude CLI)"
    echo "   Please install @github/copilot via: bun install -g @github/copilot"
    exit 1
fi

if [ $AGENT_EXIT -ne 0 ]; then
    echo "❌ Agent exited with error code $AGENT_EXIT"
    exit $AGENT_EXIT
fi

# Skip auto-merge if disabled
if [ "$BN_AUTO_MERGE" != "true" ]; then
    echo "⏭️  Auto-merge disabled, skipping merge step"
    exit 0
fi

# Attempt fast-forward merge
echo "🔀 Merging $WORK_BRANCH into $BN_MERGE_TARGET..."

# Fetch latest target branch
git fetch origin "$BN_MERGE_TARGET" 2>/dev/null || true

# Checkout target and attempt fast-forward merge
git checkout "$BN_MERGE_TARGET"
if git merge --ff-only "$WORK_BRANCH"; then
    echo "✅ Successfully merged $WORK_BRANCH into $BN_MERGE_TARGET"
else
    echo "❌ Fast-forward merge failed - manual intervention required"
    echo "   Branch $WORK_BRANCH has diverged from $BN_MERGE_TARGET"
    git checkout "$WORK_BRANCH"  # Return to work branch
    exit 1
fi

echo "🎉 Agent work complete!"
