#!/bin/bash
# Validate info panel test file loads without console errors
set -euo pipefail

BN_GUI_PORT="${BN_GUI_PORT:-9997}"
echo "Starting GUI server on port $BN_GUI_PORT for info panel testing..."

# Build and start GUI in dev mode
cargo build --features gui --quiet
./target/debug/bn gui serve --dev --port "$BN_GUI_PORT" &
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

# Test the comprehensive panel test file
echo "Testing info panel with all node types..."
CONSOLE_OUTPUT=$(LIGHTPANDA_DISABLE_TELEMETRY=true lightpanda fetch --dump "http://127.0.0.1:$BN_GUI_PORT/test-panel-all-nodes.html" 2>&1 || true)

# Check for console errors/warnings
if echo "$CONSOLE_OUTPUT" | grep '\$level=warn.*\$msg=console\.\(error\|warn\)' > /dev/null; then
    echo "❌ Info panel test FAILED - console errors detected:"
    echo "$CONSOLE_OUTPUT" | grep '\$level=warn.*\$msg=console\.\(error\|warn\)' 
    exit 1
fi

echo "✅ Info panel test passed - no console errors in test-panel-all-nodes.html"
echo ""
echo "Manual testing checklist:"
echo "  1. Open http://127.0.0.1:$BN_GUI_PORT/test-panel-all-nodes.html in a browser"
echo "  2. Test all node types (task, bug, doc, milestone, idea, agent)"
echo "  3. Verify panel does NOT overlap mock menu bar"
echo "  4. Test expand/collapse animations (smooth 250ms transition)"
echo "  5. Test keyboard accessibility (Escape, Tab navigation)"
echo "  6. Test edge cases (no edges, many edges, long description)"
echo ""
echo "Press Ctrl+C to stop the server when testing is complete"
wait $GUI_PID
