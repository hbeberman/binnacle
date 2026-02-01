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
BN_AUTO_MERGE="${BN_AUTO_MERGE:-false}"
BN_READONLY_WORKSPACE="${BN_READONLY_WORKSPACE:-false}"

cd /workspace

# Display readonly mode status at startup
if [ "$BN_READONLY_WORKSPACE" = "true" ]; then
    echo "📖 Read-only workspace mode enabled"
    echo "   - Git hooks will not be configured"
    echo "   - Auto-merge will be skipped"
fi

# Pre-populate GitHub SSH host keys to avoid interactive prompt
# Uses official GitHub SSH keys from https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints
mkdir -p ~/.ssh
if [ ! -f ~/.ssh/known_hosts ] || ! grep -q "github.com" ~/.ssh/known_hosts; then
    cat >> ~/.ssh/known_hosts << 'EOF'
github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl
github.com ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBEmKSENjQEezOmxkZMy7opKgwFB9nkt5YRrYMjNuG5N87uRgg6CLrbo5wAdT/y6v0mKV0U2w0WZ2YB/++Tpockg=
github.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQCj7ndNxQowgcQnjshcLrqPEiiphnt+VTTvDP6mHBL9j1aNUkY4Ue1gvwnGLVlOhGeYrnZaMgRK6+PKCUXaDbC7qtbW8gIkhL7aGCsOr/C56SJMy/BCZfxd1nWzAOxSDPgVsmerOBYfNqltV9/hWCqBywINIR+5dIg6JTJ72pcEpEjcYgXkE2YEFXV1JHnsKgbLWNlhScqb2UmyRkQyytRLtL+38TGxkxCflmO+5Z8CSSNY7GidjMIZ7Q4zMjA2n1nGrlTDkzwDCsw+wqFPGQA179cnfGWOWRVruj16z6XyvxvjJwbz0wQZ75XK5tKSb7FNyeIEs4TT4jk+S4dhPeAUC5y+bDYirYgM4GC7uEnztnZyaVWQ7B381AK4Qdrwt51ZqExKbQpTUNn+EjqoTwvqNj4kqx5QUCI0ThS/YkOxJCXmPUWZbhjpCg56i+2aB6CmK2JGhn57K5mj0MNdBXA4/WnwH6XoPWJzK5Nyu2zB3nAZp+S5hpQs+p1vN1/wsjk=
EOF
    echo "🔑 GitHub SSH host keys added to known_hosts"
fi

# Initialize binnacle configs:
# - Claude skills file (~/.claude/skills/binnacle/SKILL.md)
# - Codex skills file (~/.codex/skills/binnacle/SKILL.md)
# - Copilot MCP config (~/.copilot/mcp-config.json) - merges with existing
# Note: AGENTS.md is managed by the repo maintainer, not auto-generated here
echo "📝 Initializing binnacle configuration..."
if ! bn system host-init -y --write-claude-skills --write-codex-skills --write-mcp-copilot > /dev/null 2>&1; then
    echo "❌ Failed to initialize binnacle configuration"
    echo "   bn system host-init failed"
    exit 1
fi
echo "✅ Binnacle configuration initialized"

# Inject container env vars into MCP config
# Copilot MCP zeros out all env vars except PATH, so we inject ${VAR} placeholders
# that Copilot will expand when spawning the MCP server process.
#
# Note: BN_AGENT_SESSION is intentionally omitted here - it's set dynamically
# via 'export BN_AGENT_SESSION=1' later in this script (after git hooks config).
# The MCP server passes it through to subprocesses from the runtime environment.
MCP_CONFIG="$HOME/.copilot/mcp-config.json"
ENV_JSON="{}"
INJECTED_VARS=""
for var in BN_DATA_DIR BN_CONTAINER_MODE BN_STORAGE_HASH BN_AGENT_ID BN_AGENT_NAME BN_AGENT_TYPE; do
    val="${!var:-}"
    if [ -n "$val" ]; then
        ENV_JSON=$(echo "$ENV_JSON" | jq --arg k "$var" --arg v "\${$var}" '. + {($k): $v}')
        if [ -n "$INJECTED_VARS" ]; then
            INJECTED_VARS="$INJECTED_VARS, $var"
        else
            INJECTED_VARS="$var"
        fi
    fi
done

if [ "$ENV_JSON" != "{}" ]; then
    jq --argjson env "$ENV_JSON" '.mcpServers.binnacle.env = $env' "$MCP_CONFIG" > "$MCP_CONFIG.tmp"
    mv "$MCP_CONFIG.tmp" "$MCP_CONFIG"
    echo "🔌 Injected MCP env vars: $INJECTED_VARS"
fi

# Write Copilot config to enable LSP
COPILOT_CONFIG="$HOME/.copilot/config.json"
cat > "$COPILOT_CONFIG" << 'EOF'
{
  "staff": true
}
EOF
echo "🔧 Copilot config written to $COPILOT_CONFIG (staff mode enabled for LSP)"

# Write LSP configuration for Copilot code intelligence
LSP_CONFIG="$HOME/.copilot/lsp-config.json"
cat > "$LSP_CONFIG" << 'EOF'
{
  "lspServers": {
    "rust-analyzer": {
      "command": "rust-analyzer",
      "fileExtensions": {
        ".rs": "rust"
      }
    },
    "vtsls": {
      "command": "vtsls",
      "args": ["--stdio"],
      "fileExtensions": {
        ".ts": "typescript",
        ".tsx": "typescriptreact",
        ".js": "javascript",
        ".jsx": "javascriptreact",
        ".mts": "typescript",
        ".cts": "typescript",
        ".mjs": "javascript",
        ".cjs": "javascript"
      }
    }
  }
}
EOF
echo "🔧 LSP configuration written to $LSP_CONFIG"
echo "📋 Configured language servers:"
jq -r '.lspServers | keys[] as $k | "   - \($k): \(.[$k].command)"' "$LSP_CONFIG"

# Configure git hooks path so commit-msg hook adds co-author trailer
# The hooks/ directory in the repo contains the commit-msg hook that adds
# "Co-authored-by: binnacle-bot <noreply@binnacle.bot>" when BN_AGENT_SESSION=1
# Skip in readonly mode as hooks may modify the repository
if [ "$BN_READONLY_WORKSPACE" != "true" ]; then
    if [ -d "hooks" ]; then
        git config core.hooksPath hooks
        echo "🪝 Git hooks configured (core.hooksPath = hooks)"
    fi
else
    echo "⏭️  Skipping git hooks setup (readonly mode)"
fi

# Mark this as an agent session for the commit-msg hook
export BN_AGENT_SESSION=1

# Git identity: REQUIRE it from environment variables.
# The container MUST receive git identity from:
# 1. GIT_AUTHOR_*/GIT_COMMITTER_* env vars passed by the host (required)
# We do NOT set fallback defaults and do NOT allow the agent to set them.
# This prevents AI agents from polluting local git config with fake identities.
if [ -z "${GIT_AUTHOR_NAME:-}" ] || [ -z "${GIT_AUTHOR_EMAIL:-}" ]; then
    echo "❌ Git identity not provided via environment variables"
    echo "   GIT_AUTHOR_NAME=${GIT_AUTHOR_NAME:-<not set>}"
    echo "   GIT_AUTHOR_EMAIL=${GIT_AUTHOR_EMAIL:-<not set>}"
    echo ""
    echo "   This should be set automatically by 'bn container run'."
    echo "   If you're running the container directly, set these env vars."
    exit 1
fi
echo "👤 Git identity: ${GIT_AUTHOR_NAME} <${GIT_AUTHOR_EMAIL}>"

# Display build information
echo "🔨 Build info:"
bn system build-info -H | sed 's/^/   /'

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

# Print the system prompt for log visibility
echo "--- SYSTEM PROMPT ---"
echo "$BN_INITIAL_PROMPT"
echo "--- END PROMPT ---"

# Blocked MCP tools - orient/goodbye must use shell for proper agent lifecycle
BLOCKED_TOOLS=(
    --deny-tool "binnacle(binnacle-orient)"
    --deny-tool "binnacle(binnacle-goodbye)"
)

# Use container-local pinned copilot binary
# The copilot binary is pre-installed during image build at a pinned version
# to prevent auto-updates from breaking the container in production.
# Use 'bn system copilot path' to get the exact path for the embedded version
COPILOT_PATH_INFO=$(bn system copilot path 2>/dev/null || true)
COPILOT_BIN=$(echo "$COPILOT_PATH_INFO" | jq -r '.path // empty')
COPILOT_EXISTS=$(echo "$COPILOT_PATH_INFO" | jq -r '.exists // false')
COPILOT_VERSION=$(echo "$COPILOT_PATH_INFO" | jq -r '.version // empty')

if [ "$COPILOT_EXISTS" = "true" ] && [ -n "$COPILOT_BIN" ] && [ -x "$COPILOT_BIN" ]; then
    echo "🤖 Using pinned copilot $COPILOT_VERSION: $COPILOT_BIN"
    "$COPILOT_BIN" --allow-all --no-auto-update "${BLOCKED_TOOLS[@]}" -p "$BN_INITIAL_PROMPT"
    AGENT_EXIT=$?
elif command -v claude &> /dev/null; then
    echo "🤖 Using claude CLI"
    claude -p "$BN_INITIAL_PROMPT"
    AGENT_EXIT=$?
else
    echo "❌ No AI agent found (copilot or claude CLI)"
    echo "   Expected copilot at: $COPILOT_BIN"
    echo "   Run 'bn system copilot install --upstream' during container build"
    exit 1
fi

if [ $AGENT_EXIT -ne 0 ]; then
    echo "❌ Agent exited with error code $AGENT_EXIT"
    exit $AGENT_EXIT
fi

# Skip auto-merge if disabled or workspace is read-only
if [ "$BN_AUTO_MERGE" != "true" ]; then
    echo "⏭️  Auto-merge disabled, skipping merge step"
    exit 0
fi

# Skip auto-merge for read-only workspace (can't modify branches)
if [ "${BN_READONLY_WORKSPACE:-false}" = "true" ]; then
    echo "⏭️  Read-only workspace mode, skipping merge step"
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

    # Generate graph snapshot for the merge commit on main branch
    # This preserves the graph state at this point in the commit history
    MERGE_COMMIT=$(git rev-parse HEAD)
    echo "📸 Creating graph snapshot for commit $MERGE_COMMIT..."
    if bn system store archive "$MERGE_COMMIT" > /dev/null 2>&1; then
        echo "✅ Graph snapshot created"
    else
        echo "⚠️  Warning: Failed to create graph snapshot (archive.directory may not be configured)"
    fi
else
    echo "❌ Fast-forward merge failed - manual intervention required"
    echo "   Branch $WORK_BRANCH has diverged from $BN_MERGE_TARGET"
    git checkout "$WORK_BRANCH"  # Return to work branch
    exit 1
fi

echo "🎉 Agent work complete!"
