#!/bin/bash
# Binnacle Container Entrypoint (default image)
# Minimal entrypoint for the binnacle-default base image.
# Worker containers may use entrypoint.sh which adds LSP configuration.
#
# Usage:
#   ./bn-entry.sh              # Normal mode: run full entrypoint including copilot
#   ./bn-entry.sh shell        # Interactive shell mode
#   ./bn-entry.sh --source-only # Source mode: set up environment only, skip copilot
#   source ./bn-entry.sh --source-only  # Source from child script
#
# When --source-only is used, this script sets up:
#   - HOME directory
#   - nss_wrapper (user identity)
#   - Git identity validation
#   - SSH host keys
#   - Binnacle initialization
#   - Git hooks
#   - BN_AGENT_SESSION environment variable
#
# But skips:
#   - Agent orientation (bn orient)
#   - Copilot execution
#   - Auto-merge

set -eu

# === SOURCE-ONLY MODE CHECK ===
# When --source-only is passed, set up environment but don't run copilot.
# This allows child entrypoints (e.g., worker entrypoint.sh) to source this
# script and inherit the common setup without duplicating code.
BN_ENTRY_SOURCE_ONLY=false
if [ "${1:-}" = "--source-only" ]; then
    BN_ENTRY_SOURCE_ONLY=true
    shift  # Remove --source-only from args
fi

# === 1. HOME DIRECTORY SETUP ===
if [ ! -w "${HOME:-/}" ]; then
    export HOME="/tmp/agent-home"
fi
mkdir -p "$HOME"

# === 2. USER IDENTITY (nss_wrapper) ===
CURRENT_UID=$(id -u)
CURRENT_GID=$(id -g)

if [ "$CURRENT_UID" != "0" ]; then
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
    cp /etc/nss_wrapper/passwd "$NSS_WRAPPER_DIR/passwd"
    cp /etc/nss_wrapper/group "$NSS_WRAPPER_DIR/group"
    echo "agent:x:${CURRENT_UID}:${CURRENT_GID}:Binnacle Agent:${HOME}:/bin/bash" >> "$NSS_WRAPPER_DIR/passwd"
    if ! grep -Fq ":${CURRENT_GID}:" "$NSS_WRAPPER_DIR/group"; then
        echo "agent:x:${CURRENT_GID}:" >> "$NSS_WRAPPER_DIR/group"
    fi
    export LD_PRELOAD=/usr/lib64/libnss_wrapper.so
    export NSS_WRAPPER_PASSWD="$NSS_WRAPPER_DIR/passwd"
    export NSS_WRAPPER_GROUP="$NSS_WRAPPER_DIR/group"
    echo "🔧 Running as user (UID $CURRENT_UID)"
else
    echo "🔧 Running as root (--allow-sudo mode)"
fi

# === 3. GIT IDENTITY VALIDATION ===
if [ -z "${GIT_AUTHOR_NAME:-}" ] || [ -z "${GIT_AUTHOR_EMAIL:-}" ]; then
    echo "❌ Git identity not provided via environment variables"
    echo "   GIT_AUTHOR_NAME=${GIT_AUTHOR_NAME:-<not set>}"
    echo "   GIT_AUTHOR_EMAIL=${GIT_AUTHOR_EMAIL:-<not set>}"
    exit 1
fi
echo "👤 Git identity: ${GIT_AUTHOR_NAME} <${GIT_AUTHOR_EMAIL}>"

# === 4. SSH HOST KEYS ===
mkdir -p ~/.ssh
if [ ! -f ~/.ssh/known_hosts ] || ! grep -q "github.com" ~/.ssh/known_hosts; then
    cat >> ~/.ssh/known_hosts << 'SSHEOF'
github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl
github.com ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBEmKSENjQEezOmxkZMy7opKgwFB9nkt5YRrYMjNuG5N87uRgg6CLrbo5wAdT/y6v0mKV0U2w0WZ2YB/++Tpockg=
github.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQCj7ndNxQowgcQnjshcLrqPEiiphnt+VTTvDP6mHBL9j1aNUkY4Ue1gvwnGLVlOhGeYrnZaMgRK6+PKCUXaDbC7qtbW8gIkhL7aGCsOr/C56SJMy/BCZfxd1nWzAOxSDPgVsmerOBYfNqltV9/hWCqBywINIR+5dIg6JTJ72pcEpEjcYgXkE2YEFXV1JHnsKgbLWNlhScqb2UmyRkQyytRLtL+38TGxkxCflmO+5Z8CSSNY7GidjMIZ7Q4zMjA2n1nGrlTDkzwDCsw+wqFPGQA179cnfGWOWRVruj16z6XyvxvjJwbz0wQZ75XK5tKSb7FNyeIEs4TT4jk+S4dhPeAUC5y+bDYirYgM4GC7uEnztnZyaVWQ7B381AK4Qdrwt51ZqExKbQpTUNn+EjqoTwvqNj4kqx5QUCI0ThS/YkOxJCXmPUWZbhjpCg56i+2aB6CmK2JGhn57K5mj0MNdBXA4/WnwH6XoPWJzK5Nyu2zB3nAZp+S5hpQs+p1vN1/wsjk=
SSHEOF
    echo "🔑 GitHub SSH host keys added"
fi

# === 5. BINNACLE INIT ===
# host-init auto-detects BN_CONTAINER_MODE=true and applies container-specific config
echo "📝 Initializing binnacle environment..."
bn system host-init -y

# === 6. GIT HOOKS (if not readonly) ===
cd /workspace
BN_READONLY_WORKSPACE="${BN_READONLY_WORKSPACE:-false}"
if [ "$BN_READONLY_WORKSPACE" != "true" ] && [ -d "hooks" ]; then
    git config core.hooksPath hooks
    echo "🪝 Git hooks configured"
fi
export BN_AGENT_SESSION=1

# === 7. SOURCE-ONLY EXIT POINT ===
# If --source-only was passed, we've set up the environment.
# Export key variables and return control to the sourcing script.
if [ "$BN_ENTRY_SOURCE_ONLY" = "true" ]; then
    # Export variables that child scripts may need
    export HOME
    export BN_AGENT_SESSION
    export BN_READONLY_WORKSPACE
    [ -n "${LD_PRELOAD:-}" ] && export LD_PRELOAD
    [ -n "${NSS_WRAPPER_PASSWD:-}" ] && export NSS_WRAPPER_PASSWD
    [ -n "${NSS_WRAPPER_GROUP:-}" ] && export NSS_WRAPPER_GROUP
    # Return successfully - do not exit (may be sourced)
    return 0 2>/dev/null || exit 0
fi

# === 8. ORIENT AGENT ===
BN_AGENT_TYPE="${BN_AGENT_TYPE:-worker}"
echo "🧭 Orienting agent (type: $BN_AGENT_TYPE)..."
bn orient --type "$BN_AGENT_TYPE" -H

# === 9. SHELL MODE CHECK ===
if [ "${1:-}" = "shell" ] || [ "${1:-}" = "bash" ]; then
    echo "🐚 Starting interactive shell..."
    exec /bin/bash
fi

# === 10. RUN COPILOT ===
BN_INITIAL_PROMPT="${BN_INITIAL_PROMPT:-Run bn ready to see available tasks, pick one, and complete it. Call bn goodbye when done.}"

BLOCKED_TOOLS=(
    --deny-tool "binnacle(binnacle-orient)"
    --deny-tool "binnacle(binnacle-goodbye)"
)

COPILOT_PATH_INFO=$(bn system copilot path 2>/dev/null || true)
COPILOT_BIN=$(echo "$COPILOT_PATH_INFO" | jq -r '.path // empty')
COPILOT_EXISTS=$(echo "$COPILOT_PATH_INFO" | jq -r '.exists // false')
COPILOT_VERSION=$(echo "$COPILOT_PATH_INFO" | jq -r '.version // empty')

if [ "$COPILOT_EXISTS" = "true" ] && [ -n "$COPILOT_BIN" ] && [ -x "$COPILOT_BIN" ]; then
    echo "🤖 Using pinned copilot $COPILOT_VERSION"
    "$COPILOT_BIN" --allow-all --no-auto-update "${BLOCKED_TOOLS[@]}" -p "$BN_INITIAL_PROMPT"
    AGENT_EXIT=$?
else
    echo "❌ Copilot CLI not found at expected path"
    exit 1
fi

# === 11. AUTO-MERGE (if enabled) ===
BN_AUTO_MERGE="${BN_AUTO_MERGE:-false}"
BN_MERGE_TARGET="${BN_MERGE_TARGET:-main}"

if [ "$BN_AUTO_MERGE" = "true" ] && [ "$BN_READONLY_WORKSPACE" != "true" ]; then
    WORK_BRANCH=$(git rev-parse --abbrev-ref HEAD)
    echo "🔀 Merging $WORK_BRANCH into $BN_MERGE_TARGET..."
    git fetch origin "$BN_MERGE_TARGET" 2>/dev/null || true
    git checkout "$BN_MERGE_TARGET"
    if git merge --ff-only "$WORK_BRANCH"; then
        echo "✅ Merged $WORK_BRANCH into $BN_MERGE_TARGET"
        MERGE_COMMIT=$(git rev-parse HEAD)
        bn system store archive "$MERGE_COMMIT" > /dev/null 2>&1 || true
    else
        echo "❌ Fast-forward merge failed"
        git checkout "$WORK_BRANCH"
        exit 1
    fi
fi

exit ${AGENT_EXIT:-0}
