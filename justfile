# Binnacle build commands

# Build the project (debug or release)
build mode="debug" features="":
    @if [ "{{mode}}" = "release" ]; then \
        if [ -n "{{features}}" ]; then \
            cargo build --release --features {{features}}; \
        else \
            cargo build --release; \
        fi \
    else \
        if [ -n "{{features}}" ]; then \
            cargo build --features {{features}}; \
        else \
            cargo build; \
        fi \
    fi

# Install release build with all features to ~/.local/bin
install:
    cargo build --release --all-features
    mkdir -p ~/.local/bin
    cp target/release/bn ~/.local/bin/bn.tmp
    mv ~/.local/bin/bn.tmp ~/.local/bin/bn
    @echo "Installed bn to ~/.local/bin/bn (with all features)"

# Install devtunnel CLI for tunnel support
# macOS/Linux: downloads binary to ~/.local/bin
# After install, run: devtunnel user login
install-devtunnel:
    #!/usr/bin/env bash
    set -e
    
    # Check if already installed
    if command -v devtunnel &>/dev/null; then
        echo "devtunnel is already installed: $(command -v devtunnel)"
        devtunnel --version
        echo ""
        echo "Note: Run 'devtunnel user login' if you haven't authenticated yet."
        exit 0
    fi
    
    OS="$(uname -s)"
    ARCH="$(uname -m)"
    
    # Map OS/arch to devtunnel download URL
    case "$OS" in
        Darwin)
            case "$ARCH" in
                x86_64)  PLATFORM="osx-x64" ;;
                arm64)   PLATFORM="osx-arm64" ;;
                *)
                    echo "Error: Unsupported macOS architecture: $ARCH"
                    exit 1
                    ;;
            esac
            ;;
        Linux)
            case "$ARCH" in
                x86_64)  PLATFORM="linux-x64" ;;
                aarch64) PLATFORM="linux-arm64" ;;
                *)
                    echo "Error: Unsupported Linux architecture: $ARCH"
                    exit 1
                    ;;
            esac
            ;;
        *)
            echo "Error: Unsupported OS: $OS"
            echo "See: https://learn.microsoft.com/en-us/azure/developer/dev-tunnels/get-started"
            exit 1
            ;;
    esac
    
    echo "Installing devtunnel to ~/.local/bin..."
    mkdir -p ~/.local/bin
    
    URL="https://aka.ms/TunnelsCliDownload/$PLATFORM"
    echo "Downloading from: $URL"
    curl -L -o ~/.local/bin/devtunnel "$URL"
    chmod +x ~/.local/bin/devtunnel
    echo "Installed devtunnel to ~/.local/bin/devtunnel"
    ~/.local/bin/devtunnel --version
    
    echo ""
    echo "✓ devtunnel installed successfully"
    echo ""
    echo "IMPORTANT: Run 'devtunnel user login' to authenticate before using tunnels."
    echo "You can log in with GitHub, Microsoft, or Azure AD account."

# Run GUI in development mode (serves from filesystem, auto-reloads on changes)
dev-gui:
    @echo "Starting GUI in development mode..."
    cargo run --all-features -- gui --dev

gui nobuild="" tunnel="":
    #!/usr/bin/env bash
    set -e
    export BN_GUI_PORT="${BN_GUI_PORT:-55823}"
    
    # Setup paths
    RUNTIME_DIR="${XDG_RUNTIME_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}}/binnacle"
    mkdir -p "$RUNTIME_DIR"
    REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
    REPO_HASH="$(echo "$REPO_ROOT" | sha256sum | cut -c1-8)"
    GUI_BIN="$RUNTIME_DIR/bn-gui-$REPO_HASH"
    
    # Build tunnel flags if requested
    TUNNEL_FLAGS=""
    if [ -n "{{tunnel}}" ]; then
        TUNNEL_FLAGS="--tunnel"
    fi
    
    # Helper to stop existing GUI for this repo
    stop_gui() {
        if [ -f "$GUI_BIN" ]; then
            PIDS=$(fuser "$GUI_BIN" 2>/dev/null | tr -s ' ') || true
            if [ -n "$PIDS" ]; then
                echo "Stopping existing GUI for this repo (pid: $PIDS)..."
                for pid in $PIDS; do
                    kill "$pid" 2>/dev/null || true
                done
                sleep 1
            fi
        fi
    }
    
    if [ -z "{{nobuild}}" ]; then
        # Hot restart: start immediately with existing binary, then rebuild and restart
        if [ -f ~/.local/bin/bn ]; then
            stop_gui
            cp ~/.local/bin/bn "$GUI_BIN"
            echo "Starting GUI immediately with existing binary..."
            "$GUI_BIN" gui serve --host 0.0.0.0 --replace $TUNNEL_FLAGS &
            GUI_PID=$!
            echo "GUI started (pid: $GUI_PID), building new version in background..."
            
            # Build new version
            if just install; then
                echo "Build complete, restarting GUI with new binary..."
                stop_gui
                cp ~/.local/bin/bn "$GUI_BIN"
                "$GUI_BIN" gui serve --host 0.0.0.0 --replace $TUNNEL_FLAGS
            else
                echo "Build failed, keeping existing GUI running"
                wait $GUI_PID
            fi
        else
            # No existing binary, must build first
            echo "No existing binary found, building first..."
            just install
            stop_gui
            cp ~/.local/bin/bn "$GUI_BIN"
            "$GUI_BIN" gui serve --host 0.0.0.0 --replace $TUNNEL_FLAGS
        fi
    else
        echo "Skipping build (using existing binary)..."
        stop_gui
        cp ~/.local/bin/bn "$GUI_BIN"
        "$GUI_BIN" gui serve --host 0.0.0.0 --replace $TUNNEL_FLAGS
    fi

# Run clippy with strict warnings
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Quick validation: format check and clippy
check:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
    cargo test --all-features

# Run shell-based tests (bn-agent script tests)
test-shell:
    bash tests/run-shell-tests.sh

# Run all tests (cargo + shell)
test-all: test test-shell
    @echo ""
    @echo "✓ All tests passed!"


# Run the development build (explicitly uses ./target, not system bn)
# Usage: just dev orient, just dev task list, just dev --help
dev *args:
    cargo run -- {{args}}

# Build WASM module with wasm-pack
# Requires: wasm-pack (cargo install wasm-pack)
# Output: pkg/ directory with JS bindings
build-wasm:
    wasm-pack build --target web --features wasm

# Build WASM module in release mode with optimizations
build-wasm-release:
    wasm-pack build --target web --features wasm --release

# Build self-contained viewer.html with embedded WASM
# Requires: wasm-pack, python3
# Output: target/viewer/viewer.html
build-viewer:
    ./scripts/embed_wasm.sh

# Build viewer in release mode with optimized WASM
build-viewer-release:
    ./scripts/embed_wasm.sh --release

# Build npm package for @binnacle/viewer
# Requires: just build-viewer-release first
# Output: npm/viewer.html (ready for npm publish)
npm-package:
    #!/usr/bin/env bash
    set -e
    if [ ! -f target/viewer/viewer.html ]; then
        echo "Error: target/viewer/viewer.html not found"
        echo "Run 'just build-viewer-release' first"
        exit 1
    fi
    cp target/viewer/viewer.html npm/viewer.html
    echo "✓ Copied viewer.html to npm/"
    echo "Package ready at npm/"
    echo "To publish: cd npm && npm publish --access public"

# Build viewer and prepare npm package in one step
npm-build: build-viewer-release npm-package

# Serve the bundled WASM viewer locally
# Usage: just serve-wasm [port] [path-to-bng]
# Examples:
#   just serve-wasm                    # Serve on port 8080, no archive
#   just serve-wasm 3000               # Serve on port 3000
#   just serve-wasm 8080 /path/to.bng  # Pre-load a .bng archive
serve-wasm port="8080" archive="":
    #!/usr/bin/env bash
    set -e
    
    VIEWER_PATH="target/viewer/viewer.html"
    
    # Check if viewer exists, offer to build if not
    if [ ! -f "$VIEWER_PATH" ]; then
        echo "Viewer not found at $VIEWER_PATH"
        echo "Building viewer first..."
        just build-viewer
    fi
    
    # Create a temp directory to serve from
    SERVE_DIR=$(mktemp -d)
    trap "rm -rf '$SERVE_DIR'" EXIT
    
    # Copy viewer.html to the temp directory as index.html
    cp "$VIEWER_PATH" "$SERVE_DIR/index.html"
    
    # If archive provided, copy it and create a redirect
    ARCHIVE_PARAM=""
    if [ -n "{{archive}}" ]; then
        ARCHIVE_FILE="{{archive}}"
        if [ ! -f "$ARCHIVE_FILE" ]; then
            echo "Error: Archive not found: $ARCHIVE_FILE"
            exit 1
        fi
        ARCHIVE_NAME=$(basename "$ARCHIVE_FILE")
        cp "$ARCHIVE_FILE" "$SERVE_DIR/$ARCHIVE_NAME"
        ARCHIVE_PARAM="?file=$ARCHIVE_NAME"
        echo "Archive available at: http://localhost:{{port}}/$ARCHIVE_NAME"
    fi
    
    echo "Serving WASM viewer at: http://localhost:{{port}}/$ARCHIVE_PARAM"
    echo "Press Ctrl+C to stop"
    
    # Use Python's built-in HTTP server (most portable)
    cd "$SERVE_DIR" && python3 -m http.server {{port}}

# Bundle web assets (JS/CSS) and compress with zstd
bundle-web:
    ./scripts/bundle-web.sh

# Validate web portal with lightpanda (check for JS errors)
# Checks for lightpanda binary and validates GUI loads without console errors
gui-check:
    #!/usr/bin/env bash
    set -e
    # Check if lightpanda is installed
    if ! command -v lightpanda &> /dev/null; then
        echo "❌ lightpanda not found!"
        echo "Install lightpanda v0.2.1 from: https://github.com/lightpanda-io/browser/releases"
        echo ""
        echo "Quick install (Linux x86_64):"
        echo "  curl -L -o lightpanda https://github.com/lightpanda-io/browser/releases/download/v0.2.1/lightpanda-x86_64-linux"
        echo "  chmod +x lightpanda && sudo mv lightpanda /usr/local/bin/"
        exit 1
    fi
    ./scripts/gui-check.sh

# Build the container image (builds release binary first)
container tag="binnacle-self:latest":
    @echo "Building release binary..."
    cargo build --release
    @echo "Copying binary to container/bn..."
    cp target/release/bn container/bn
    @echo "Building container image..."
    podman build -t {{tag}} -f container/Containerfile .
    @rm -f container/bn
    @echo "Container image built: {{tag}}"
