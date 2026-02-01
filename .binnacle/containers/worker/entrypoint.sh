#!/bin/bash
# Binnacle Container Worker Entrypoint
# This script extends the base bn-entry.sh with worker-specific configuration:
#   - MCP environment variable injection
#   - LSP configuration (rust-analyzer, vtsls)
#   - Copilot staff mode config
#
# Usage:
#   ./entrypoint.sh         # Run agent with LSP support
#   ./entrypoint.sh shell   # Interactive shell mode

set -eu

# === 1. SOURCE BASE ENTRYPOINT ===
# bn-entry.sh --source-only sets up:
#   - HOME directory
#   - nss_wrapper (user identity)
#   - Git identity validation
#   - SSH host keys
#   - Binnacle initialization (bn system host-init)
#   - Git hooks
#   - BN_AGENT_SESSION environment variable
# shellcheck source=/bn-entry.sh
source /bn-entry.sh --source-only

# === 2. WORKER-SPECIFIC: MCP CONFIG EXTENSIONS ===
# Override binnacle init with additional skills files for worker
echo "📝 Extending binnacle configuration for worker..."
if ! bn system host-init -y --write-claude-skills --write-codex-skills --write-mcp-copilot > /dev/null 2>&1; then
    echo "❌ Failed to extend binnacle configuration"
    exit 1
fi
echo "✅ Binnacle configuration extended"

# === 3. WORKER-SPECIFIC: MCP ENV INJECTION ===
# Inject container env vars into MCP config
# Copilot MCP zeros out all env vars except PATH, so we inject ${VAR} placeholders
# that Copilot will expand when spawning the MCP server process.
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

# === 4. WORKER-SPECIFIC: COPILOT CONFIG ===
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
jq -r '.lspServers | keys[] as $k | "   - \($k): \(.[$k].command)"' "$LSP_CONFIG"

# === 5. ORIENT AGENT ===
BN_AGENT_TYPE="${BN_AGENT_TYPE:-worker}"
echo "🧭 Orienting agent (type: $BN_AGENT_TYPE)..."
bn orient --type "$BN_AGENT_TYPE" -H

# Display build information
echo "🔨 Build info:"
bn system build-info -H | sed 's/^/   /'

# Get current branch for merge later
WORK_BRANCH=$(git rev-parse --abbrev-ref HEAD)
echo "📍 Working on branch: $WORK_BRANCH"

# === 6. SHELL MODE CHECK ===
# Use ${1:-} to handle unset $1 with set -u
if [ "${1:-}" = "shell" ] || [ "${1:-}" = "bash" ]; then
    echo "🐚 Starting interactive shell..."
    exec /bin/bash
fi

# === 7. RUN COPILOT ===
BN_INITIAL_PROMPT="${BN_INITIAL_PROMPT:-Run bn ready to see available tasks, pick one, and complete it. Call bn goodbye when done.}"
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
    exit 1
fi

if [ $AGENT_EXIT -ne 0 ]; then
    echo "❌ Agent exited with error code $AGENT_EXIT"
    exit $AGENT_EXIT
fi

# === 8. AUTO-MERGE (if enabled) ===
BN_AUTO_MERGE="${BN_AUTO_MERGE:-false}"
BN_MERGE_TARGET="${BN_MERGE_TARGET:-main}"

if [ "$BN_AUTO_MERGE" != "true" ]; then
    echo "⏭️  Auto-merge disabled, skipping merge step"
    exit 0
fi

if [ "$BN_READONLY_WORKSPACE" = "true" ]; then
    echo "⏭️  Read-only workspace mode, skipping merge step"
    exit 0
fi

# Attempt fast-forward merge
echo "🔀 Merging $WORK_BRANCH into $BN_MERGE_TARGET..."
git fetch origin "$BN_MERGE_TARGET" 2>/dev/null || true
git checkout "$BN_MERGE_TARGET"
if git merge --ff-only "$WORK_BRANCH"; then
    echo "✅ Successfully merged $WORK_BRANCH into $BN_MERGE_TARGET"
    MERGE_COMMIT=$(git rev-parse HEAD)
    echo "📸 Creating graph snapshot for commit $MERGE_COMMIT..."
    bn system store archive "$MERGE_COMMIT" > /dev/null 2>&1 || true
else
    echo "❌ Fast-forward merge failed - manual intervention required"
    git checkout "$WORK_BRANCH"
    exit 1
fi

echo "🎉 Agent work complete!"
