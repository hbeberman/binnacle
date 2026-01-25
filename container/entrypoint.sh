#!/bin/bash
# Binnacle Container Worker Entrypoint
# This script initializes the agent environment and runs the AI agent

set -eu

# Set up writable HOME for non-root user (git config, tool caches, etc.)
if [ ! -w "${HOME:-/}" ]; then
    export HOME="/tmp/agent-home"
fi
mkdir -p "$HOME"
# Ensure we own the HOME directory (may exist from previous run)
if [ ! -O "$HOME" ]; then
    echo "⚠️  Warning: $HOME exists but is not owned by current user (UID $(id -u))"
    echo "   Some tools may fail to write config files"
fi

# Handle user identity based on mode:
# - --allow-sudo mode: Running as root (UID 0), no setup needed
# - Default mode: Running as host user, need nss_wrapper for identity
CURRENT_UID=$(id -u)
CURRENT_GID=$(id -g)

if [ "$CURRENT_UID" != "0" ]; then
    # Default mode: set up nss_wrapper for user identity
    # This provides user info for Node.js os.userInfo(), git, etc.

    # Validate nss_wrapper library and template files exist
    if [ ! -f /usr/lib64/libnss_wrapper.so ]; then
        echo "❌ nss_wrapper library not found at /usr/lib64/libnss_wrapper.so"
        echo "   The container image may be corrupted. Rebuild with 'bn container build'"
        exit 1
    fi
    if [ ! -f /etc/nss_wrapper/passwd ] || [ ! -f /etc/nss_wrapper/group ]; then
        echo "❌ nss_wrapper template files not found in /etc/nss_wrapper/"
        echo "   The container image may be corrupted. Rebuild with 'bn container build'"
        exit 1
    fi

    NSS_WRAPPER_DIR="$HOME/.nss_wrapper"
    mkdir -p "$NSS_WRAPPER_DIR"

    # Copy base files and add our user
    cp /etc/nss_wrapper/passwd "$NSS_WRAPPER_DIR/passwd"
    cp /etc/nss_wrapper/group "$NSS_WRAPPER_DIR/group"

    echo "agent:x:${CURRENT_UID}:${CURRENT_GID}:Binnacle Agent:${HOME}:/bin/bash" >> "$NSS_WRAPPER_DIR/passwd"

    # Use grep -F for literal string match (GID could theoretically contain regex chars)
    if ! grep -Fq ":${CURRENT_GID}:" "$NSS_WRAPPER_DIR/group"; then
        echo "agent:x:${CURRENT_GID}:" >> "$NSS_WRAPPER_DIR/group"
    fi

    export LD_PRELOAD=/usr/lib64/libnss_wrapper.so
    export NSS_WRAPPER_PASSWD="$NSS_WRAPPER_DIR/passwd"
    export NSS_WRAPPER_GROUP="$NSS_WRAPPER_DIR/group"

    echo "🔧 Running as user (UID $CURRENT_UID) - sudo not available"
else
    echo "🔧 Running as root (--allow-sudo mode) - sudo available"
fi

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
# Use ${1:-} to handle unset $1 with set -u
if [ "${1:-}" = "shell" ] || [ "${1:-}" = "bash" ]; then
    echo "🐚 Starting interactive shell..."
    exec /bin/bash
fi

# Run the AI agent
echo "🤖 Starting AI agent..."
if command -v copilot &> /dev/null; then
    copilot --allow-all -p "$BN_INITIAL_PROMPT"
    AGENT_EXIT=$?
elif command -v claude &> /dev/null; then
    claude -p "$BN_INITIAL_PROMPT"
    AGENT_EXIT=$?
else
    echo "❌ No AI agent found (copilot or claude CLI)"
    echo "   Please install @github/copilot via: npm install -g @github/copilot"
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
