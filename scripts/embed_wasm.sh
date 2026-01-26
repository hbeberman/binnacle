#!/usr/bin/env bash
# embed_wasm.sh - Build self-contained viewer.html with embedded WASM
#
# Usage: ./scripts/embed_wasm.sh [--release] [--skip-build]
#
# This script:
# 1. Builds the WASM module using wasm-pack (unless --skip-build)
# 2. Base64 encodes the .wasm binary
# 3. Embeds the wasm-bindgen JS glue code
# 4. Produces a self-contained viewer.html
#
# Output: target/viewer/viewer.html

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse arguments
RELEASE_MODE=false
SKIP_BUILD=false
for arg in "$@"; do
    case $arg in
        --release) RELEASE_MODE=true ;;
        --skip-build) SKIP_BUILD=true ;;
        *) echo "Unknown argument: $arg"; exit 1 ;;
    esac
done

# Paths
SRC_VIEWER="$PROJECT_ROOT/src/wasm/viewer.html"
PKG_DIR="$PROJECT_ROOT/pkg"
OUTPUT_DIR="$PROJECT_ROOT/target/viewer"
OUTPUT_FILE="$OUTPUT_DIR/viewer.html"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }

# Step 1: Build WASM module
if ! $SKIP_BUILD; then
    log_info "Building WASM module..."
    if $RELEASE_MODE; then
        wasm-pack build "$PROJECT_ROOT" --target web --release -- --features wasm
    else
        wasm-pack build "$PROJECT_ROOT" --target web -- --features wasm
    fi
fi

# Verify wasm-pack output exists
WASM_FILE="$PKG_DIR/binnacle_bg.wasm"
JS_FILE="$PKG_DIR/binnacle.js"

if [[ ! -f "$WASM_FILE" ]]; then
    log_error "WASM file not found: $WASM_FILE"
    log_error "Run without --skip-build or ensure wasm-pack build succeeded"
    exit 1
fi

if [[ ! -f "$JS_FILE" ]]; then
    log_error "JS glue file not found: $JS_FILE"
    exit 1
fi

# Step 2: Base64 encode WASM binary
log_info "Encoding WASM binary ($(du -h "$WASM_FILE" | cut -f1))..."
WASM_BASE64=$(base64 -w0 "$WASM_FILE")
WASM_BASE64_SIZE=$(echo -n "$WASM_BASE64" | wc -c)
log_info "Base64 encoded size: $(echo "$WASM_BASE64_SIZE" | awk '{printf "%.1f KB", $1/1024}')"

# Step 3: Read JS glue code
log_info "Reading JS glue code..."

# Step 4: Create output directory
mkdir -p "$OUTPUT_DIR"

# Step 5: Get version from Cargo.toml
VERSION=$(grep '^version' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

# Step 6: Write base64 to temp file for Python to read
WASM_BASE64_FILE=$(mktemp)
echo -n "$WASM_BASE64" > "$WASM_BASE64_FILE"
trap "rm -f '$WASM_BASE64_FILE'" EXIT

# Step 7: Generate the embedded viewer.html using Python for reliable string handling
log_info "Generating embedded viewer.html..."

python3 << PYTHON_SCRIPT
import sys
import re

src_viewer = "$SRC_VIEWER"
js_file = "$JS_FILE"
output_file = "$OUTPUT_FILE"
version = "$VERSION"
wasm_base64_file = "$WASM_BASE64_FILE"

# Read WASM base64 from temp file
with open(wasm_base64_file, 'r') as f:
    wasm_base64 = f.read().strip()

# Read template
with open(src_viewer, 'r') as f:
    template = f.read()

# Read JS glue
with open(js_file, 'r') as f:
    js_glue = f.read()

# The embedded WASM initialization code
# This replaces the try/catch block in initWasm()
embedded_init = f'''
            // ==========================================
            // EMBEDDED WASM MODULE
            // ==========================================
            
            // WASM binary is base64 encoded and decoded at runtime
            const WASM_BASE64 = "{wasm_base64}";
            
            // Decode base64 to bytes
            function base64ToBytes(base64) {{
                const binStr = atob(base64);
                const bytes = new Uint8Array(binStr.length);
                for (let i = 0; i < binStr.length; i++) {{
                    bytes[i] = binStr.charCodeAt(i);
                }}
                return bytes;
            }}
            
            // Embedded wasm-bindgen JS glue
            const createWasmModule = (function() {{
                // --- BEGIN WASM-BINDGEN GLUE ---
{js_glue}
                // --- END WASM-BINDGEN GLUE ---
                
                // Return the init function and exports
                return {{ init: __wbg_init, BinnacleViewer, version }};
            }})();
            
            // Initialize with embedded bytes
            const wasmBytes = base64ToBytes(WASM_BASE64);
            await createWasmModule.init(wasmBytes);
            return createWasmModule;
'''

# Pattern to match the placeholder comment through the throw statement
pattern = r'// __WASM_INIT_PLACEHOLDER__.*?throw new Error\([\'"]WASM module not available[^\)]*\);'

# Use DOTALL to match across newlines
new_content = re.sub(pattern, embedded_init.strip(), template, flags=re.DOTALL)

# Verify replacement happened
if '__WASM_INIT_PLACEHOLDER__' in new_content:
    print("ERROR: Placeholder replacement failed!", file=sys.stderr)
    sys.exit(1)

# Write output
with open(output_file, 'w') as f:
    f.write(new_content)

print(f"Generated viewer.html with embedded WASM ({len(wasm_base64)} bytes base64)")
PYTHON_SCRIPT

# Step 7: Report results
WASM_SIZE=$(stat -c%s "$WASM_FILE" 2>/dev/null || stat -f%z "$WASM_FILE")
OUTPUT_SIZE=$(stat -c%s "$OUTPUT_FILE" 2>/dev/null || stat -f%z "$OUTPUT_FILE")

log_info "Build complete!"
log_info "  WASM binary:  $(numfmt --to=iec "$WASM_SIZE" 2>/dev/null || echo "$WASM_SIZE bytes")"
log_info "  Output file:  $OUTPUT_FILE"
log_info "  Output size:  $(numfmt --to=iec "$OUTPUT_SIZE" 2>/dev/null || echo "$OUTPUT_SIZE bytes")"
log_info "  Version:      $VERSION"

# Verify output doesn't contain placeholder
if grep -q "__WASM_INIT_PLACEHOLDER__" "$OUTPUT_FILE"; then
    log_error "Placeholder still present - embedding failed!"
    exit 1
fi

log_info "✓ Viewer ready: $OUTPUT_FILE"
echo ""
log_info "To run the viewer:"
echo "  just serve-wasm              # Serve on http://localhost:8080"
echo "  just serve-wasm 3000         # Serve on custom port"
echo "  just serve-wasm 8080 file.bng   # Pre-load a .bng archive"
