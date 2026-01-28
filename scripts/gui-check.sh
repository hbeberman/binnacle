#!/bin/bash
# Validate GUI loads without console errors using Lightpanda
set -euo pipefail

# Start binnacle GUI server in background
BN_GUI_PORT="${BN_GUI_PORT:-9876}"
echo "Starting GUI server on port $BN_GUI_PORT..."

# Build and start GUI (uses cargo run to avoid needing installed binary)
cargo build --features gui --quiet
./target/debug/bn gui serve --port "$BN_GUI_PORT" &
GUI_PID=$!

# Cleanup on exit
cleanup() {
    kill $GUI_PID 2>/dev/null || true
}
trap cleanup EXIT

# Wait for server to be ready
for i in {1..30}; do
    if curl -s "http://127.0.0.1:$BN_GUI_PORT" > /dev/null 2>&1; then
        break
    fi
    sleep 0.5
done

# Run Lightpanda to fetch page and capture console output
# CRITICAL: Always disable telemetry explicitly at invocation (belt-and-suspenders)
echo "Validating GUI with Lightpanda..."
CONSOLE_OUTPUT=$(LIGHTPANDA_DISABLE_TELEMETRY=true lightpanda fetch --dump "http://127.0.0.1:$BN_GUI_PORT" 2>&1 || true)

# Check for console errors/warnings
# Lightpanda outputs errors to stderr with prefixes like "error:" or "TypeError:"
if echo "$CONSOLE_OUTPUT" | grep -iE '(console\.error|console\.warn|TypeError|ReferenceError|SyntaxError|error\(|warn\()' > /dev/null; then
    echo "❌ GUI validation FAILED - console errors detected:"
    echo "$CONSOLE_OUTPUT" | grep -iE '(console\.error|console\.warn|TypeError|ReferenceError|SyntaxError|error\(|warn\())' 
    exit 1
fi

echo "✅ GUI validation passed - no console errors"
