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
    cp ~/.local/bin/bn "$CACHE_DIR/bn-gui"
    "$CACHE_DIR/bn-gui" gui --host 0.0.0.0 --replace

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
