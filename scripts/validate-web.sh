#!/usr/bin/env bash
# Validate web portal using lightpanda headless browser
# Detects JavaScript errors and warnings in console

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if lightpanda is installed
if ! command -v lightpanda &> /dev/null; then
    echo -e "${RED}❌ lightpanda not found!${NC}"
    echo "Install lightpanda v0.2.0 from: https://github.com/lightpanda-io/lightpanda/releases"
    echo ""
    echo "Quick install (Linux x86_64):"
    echo "  curl -L https://github.com/lightpanda-io/lightpanda/releases/download/v0.2.0/lightpanda-v0.2.0-x86_64-unknown-linux-gnu.tar.gz | tar xz"
    echo "  sudo mv lightpanda /usr/local/bin/"
    echo ""
    echo "Or download for your platform:"
    echo "  https://github.com/lightpanda-io/lightpanda/releases/tag/v0.2.0"
    exit 1
fi

# Disable telemetry
export LIGHTPANDA_DISABLE_TELEMETRY=true

# Configuration
GUI_PORT="${BN_GUI_PORT:-3030}"
GUI_URL="http://localhost:${GUI_PORT}"
STARTED_GUI=0

echo "Checking if GUI is running on port ${GUI_PORT}..."

# Check if GUI server is running
if ! curl -s "${GUI_URL}" > /dev/null 2>&1; then
    echo "GUI not running, starting it..."
    
    # Check if bn is available
    if ! command -v bn &> /dev/null; then
        echo -e "${RED}❌ bn command not found!${NC}"
        echo "Install bn first: just install"
        exit 1
    fi
    
    # Start GUI in background (detached)
    nohup bn gui serve --host 0.0.0.0 --port "${GUI_PORT}" --replace > /dev/null 2>&1 &
    GUI_PID=$!
    STARTED_GUI=1
    
    # Wait for server to start (max 10 seconds)
    for i in {1..20}; do
        if curl -s "${GUI_URL}" > /dev/null 2>&1; then
            echo "GUI server started successfully (pid: $GUI_PID)"
            break
        fi
        if [ $i -eq 20 ]; then
            echo -e "${RED}❌ Failed to start GUI server${NC}"
            kill $GUI_PID 2>/dev/null || true
            exit 1
        fi
        sleep 0.5
    done
else
    echo "GUI already running on port ${GUI_PORT}"
fi

# Create a temporary directory for scripts
TEMP_DIR=$(mktemp -d)
trap "rm -rf '$TEMP_DIR'" EXIT

# Create validation script
SCRIPT_FILE="$TEMP_DIR/validate.js"
cat > "$SCRIPT_FILE" << 'EOF'
// Validation script for lightpanda
// Captures console errors and warnings

const errors = [];
const warnings = [];
const logs = [];

// Override console methods to capture messages
const originalError = console.error;
const originalWarn = console.warn;
const originalLog = console.log;
const originalInfo = console.info;

console.error = function(...args) {
    const msg = args.map(a => String(a)).join(' ');
    errors.push(msg);
    originalError.apply(console, arguments);
};

console.warn = function(...args) {
    const msg = args.map(a => String(a)).join(' ');
    warnings.push(msg);
    originalWarn.apply(console, arguments);
};

// Don't treat console.log/info as errors (they're allowed)
console.log = originalLog;
console.info = originalInfo;

// Capture uncaught errors
window.addEventListener('error', function(event) {
    errors.push('Uncaught: ' + event.message + ' at ' + event.filename + ':' + event.lineno);
});

// Capture unhandled promise rejections
window.addEventListener('unhandledrejection', function(event) {
    errors.push('Unhandled rejection: ' + (event.reason ? event.reason.toString() : 'unknown'));
});

// Wait for page to fully load, then report results
window.addEventListener('load', function() {
    // Give async operations time to complete
    setTimeout(function() {
        // Print results marker for parsing
        console.log('__VALIDATION_START__');
        console.log(JSON.stringify({
            errors: errors,
            warnings: warnings,
            url: window.location.href
        }, null, 2));
        console.log('__VALIDATION_END__');
    }, 3000);
});
EOF

echo "Running lightpanda validation..."
echo "  URL: ${GUI_URL}"

# Run lightpanda and capture output
OUTPUT_FILE="$TEMP_DIR/output.txt"
if lightpanda --script="$SCRIPT_FILE" "${GUI_URL}" > "$OUTPUT_FILE" 2>&1; then
    LIGHTPANDA_EXIT=0
else
    LIGHTPANDA_EXIT=$?
fi

# Extract validation results
if grep -q '__VALIDATION_START__' "$OUTPUT_FILE"; then
    RESULT=$(sed -n '/__VALIDATION_START__/,/__VALIDATION_END__/p' "$OUTPUT_FILE" | grep -v '__VALIDATION_' || true)
else
    echo -e "${RED}❌ Failed to extract validation results${NC}"
    echo "Lightpanda output:"
    cat "$OUTPUT_FILE"
    exit 1
fi

# Parse JSON using Python (more reliable than shell)
PARSE_SCRIPT="$TEMP_DIR/parse.py"
cat > "$PARSE_SCRIPT" << 'PYEOF'
import json
import sys

try:
    data = json.load(sys.stdin)
    errors = data.get('errors', [])
    warnings = data.get('warnings', [])
    
    # Print error count
    print(f"ERRORS:{len(errors)}")
    for err in errors:
        print(f"ERROR:{err}")
    
    # Print warning count
    print(f"WARNINGS:{len(warnings)}")
    for warn in warnings:
        print(f"WARNING:{warn}")
        
except Exception as e:
    print(f"PARSE_ERROR:{e}", file=sys.stderr)
    sys.exit(1)
PYEOF

# Parse results
PARSED=$(echo "$RESULT" | python3 "$PARSE_SCRIPT" 2>&1 || echo "PARSE_ERROR:Failed to parse JSON")

# Check if parsing failed
if echo "$PARSED" | grep -q "PARSE_ERROR:"; then
    echo -e "${RED}❌ Failed to parse validation results${NC}"
    echo "Raw output:"
    echo "$RESULT"
    exit 1
fi

# Extract counts
ERROR_COUNT=$(echo "$PARSED" | grep "^ERRORS:" | cut -d: -f2)
WARNING_COUNT=$(echo "$PARSED" | grep "^WARNINGS:" | cut -d: -f2)

# Display results
echo ""
if [ "$ERROR_COUNT" -gt 0 ]; then
    echo -e "${RED}❌ JavaScript errors detected (${ERROR_COUNT}):${NC}"
    echo "$PARSED" | grep "^ERROR:" | cut -d: -f2- | sed 's/^/  /'
    echo ""
fi

if [ "$WARNING_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}⚠️  JavaScript warnings detected (${WARNING_COUNT}):${NC}"
    echo "$PARSED" | grep "^WARNING:" | cut -d: -f2- | sed 's/^/  /'
    echo ""
fi

# Determine exit code
if [ "$ERROR_COUNT" -eq 0 ] && [ "$WARNING_COUNT" -eq 0 ]; then
    echo -e "${GREEN}✅ No JavaScript errors or warnings detected${NC}"
    exit 0
else
    echo -e "${RED}❌ Validation failed${NC}"
    echo "Run 'just gui-check' to validate interactively"
    exit 1
fi
