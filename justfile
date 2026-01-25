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
    
    if [ -z "{{nobuild}}" ]; then
        just install
    else
        echo "Skipping build (using existing binary)..."
    fi
    # Copy to temp location so builds can replace the original while GUI runs
    # Use XDG_RUNTIME_DIR (tmpfs, session-scoped) with fallback to cache
    RUNTIME_DIR="${XDG_RUNTIME_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}}/binnacle"
    mkdir -p "$RUNTIME_DIR"
    
    # Use repo-specific binary to avoid killing other repos' GUI sessions
    # Hash the git root path to get a stable identifier for this repo
    REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
    REPO_HASH="$(echo "$REPO_ROOT" | sha256sum | cut -c1-8)"
    GUI_BIN="$RUNTIME_DIR/bn-gui-$REPO_HASH"
    
    # Only kill THIS repo's GUI process (not other repos)
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
    cp ~/.local/bin/bn "$GUI_BIN"
    "$GUI_BIN" gui serve --host 0.0.0.0 --replace

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
