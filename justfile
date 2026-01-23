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

gui:
    #!/usr/bin/env bash
    set -e
    export BN_GUI_PORT="${BN_GUI_PORT:-3030}"
    
    just install
    # Copy to temp location so builds can replace the original while GUI runs
    CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/binnacle"
    mkdir -p "$CACHE_DIR"
    GUI_BIN="$CACHE_DIR/bn-gui"
    # Kill any process using the cached binary (could be from any repo)
    if [ -f "$GUI_BIN" ]; then
        PIDS=$(fuser "$GUI_BIN" 2>/dev/null | tr -s ' ') || true
        if [ -n "$PIDS" ]; then
            echo "Stopping existing GUI process(es): $PIDS"
            for pid in $PIDS; do
                kill "$pid" 2>/dev/null || true
            done
            sleep 1
        fi
    fi
    cp ~/.local/bin/bn "$GUI_BIN"
    "$GUI_BIN" gui --host 0.0.0.0 --replace

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
