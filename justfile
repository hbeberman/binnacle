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

# Install release build with GUI to ~/.local/bin
install: (build "release" "gui")
    mkdir -p ~/.local/bin
    cp target/release/bn ~/.local/bin/
    @echo "Installed bn to ~/.local/bin/bn (with GUI feature)"

gui nobuild="":
    #!/usr/bin/env bash
    set -e
    export BN_GUI_PORT="${BN_GUI_PORT:-3030}"
    
    # Setup paths
    RUNTIME_DIR="${XDG_RUNTIME_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}}/binnacle"
    mkdir -p "$RUNTIME_DIR"
    REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
    REPO_HASH="$(echo "$REPO_ROOT" | sha256sum | cut -c1-8)"
    GUI_BIN="$RUNTIME_DIR/bn-gui-$REPO_HASH"
    
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
            "$GUI_BIN" gui serve --host 0.0.0.0 --replace &
            GUI_PID=$!
            echo "GUI started (pid: $GUI_PID), building new version in background..."
            
            # Build new version
            if just install; then
                echo "Build complete, restarting GUI with new binary..."
                stop_gui
                cp ~/.local/bin/bn "$GUI_BIN"
                "$GUI_BIN" gui serve --host 0.0.0.0 --replace
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
            "$GUI_BIN" gui serve --host 0.0.0.0 --replace
        fi
    else
        echo "Skipping build (using existing binary)..."
        stop_gui
        cp ~/.local/bin/bn "$GUI_BIN"
        "$GUI_BIN" gui serve --host 0.0.0.0 --replace
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

# Build the container image (builds release binary first)
container tag="binnacle-worker:latest":
    @echo "Building release binary..."
    cargo build --release
    @echo "Copying binary to container/bn..."
    cp target/release/bn container/bn
    @echo "Building container image..."
    podman build -t {{tag}} -f container/Containerfile .
    @rm -f container/bn
    @echo "Container image built: {{tag}}"
