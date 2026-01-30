#!/bin/bash
# Git wrapper that blocks --no-verify in container environments
# Agents must NEVER bypass commit hooks - they ensure code quality
#
# This wrapper is installed at /usr/local/bin/git in the container,
# taking precedence over /usr/bin/git due to PATH ordering.

# Find the real git binary (should be /usr/bin/git)
REAL_GIT="/usr/bin/git"

if [ ! -x "$REAL_GIT" ]; then
    echo "❌ Error: Real git binary not found at $REAL_GIT" >&2
    exit 1
fi

# Check if we're in container mode and the command is commit
if [ "${BN_CONTAINER_MODE:-}" = "true" ]; then
    # Build array of arguments to check
    COMMAND=""
    HAS_NO_VERIFY=false
    
    for arg in "$@"; do
        # First non-flag argument is the git command
        if [ -z "$COMMAND" ] && [[ "$arg" != -* ]]; then
            COMMAND="$arg"
        fi
        
        # Check for --no-verify or -n (short form)
        if [ "$arg" = "--no-verify" ] || [ "$arg" = "-n" ]; then
            # -n for commit means --no-verify, but for other commands it might mean something else
            # Only block for commit command
            HAS_NO_VERIFY=true
        fi
    done
    
    # Block --no-verify for commit and push commands
    if [ "$HAS_NO_VERIFY" = true ]; then
        if [ "$COMMAND" = "commit" ] || [ "$COMMAND" = "push" ]; then
            echo "" >&2
            echo "╔════════════════════════════════════════════════════════════════╗" >&2
            echo "║  ❌ ERROR: --no-verify is BLOCKED in container environments    ║" >&2
            echo "╠════════════════════════════════════════════════════════════════╣" >&2
            echo "║                                                                ║" >&2
            echo "║  Commit hooks ensure code quality and MUST run:                ║" >&2
            echo "║    • cargo fmt --check (formatting)                            ║" >&2
            echo "║    • cargo clippy (linting)                                    ║" >&2
            echo "║    • cargo audit (security)                                    ║" >&2
            echo "║                                                                ║" >&2
            echo "║  If hooks fail, FIX THE ISSUES instead of bypassing them.      ║" >&2
            echo "║                                                                ║" >&2
            echo "║  Commands to fix common issues:                                ║" >&2
            echo "║    cargo fmt          # Fix formatting                         ║" >&2
            echo "║    cargo clippy --fix # Auto-fix some lint issues              ║" >&2
            echo "║                                                                ║" >&2
            echo "╚════════════════════════════════════════════════════════════════╝" >&2
            echo "" >&2
            exit 1
        fi
    fi
fi

# Pass through to real git
exec "$REAL_GIT" "$@"
