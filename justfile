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
    
    # Check if port is already in use by a binnacle GUI
    if lsof -i :$BN_GUI_PORT -sTCP:LISTEN >/dev/null 2>&1; then
        echo "⚠️  Port $BN_GUI_PORT is already in use."
        echo "   A binnacle GUI may already be running - check your browser at http://127.0.0.1:$BN_GUI_PORT"
        echo "   To restart: kill the existing process and run 'just gui' again"
        echo "   To use a different port: BN_GUI_PORT=3031 just gui"
        exit 1
    fi
    
    just install
    # Copy to temp location so builds can replace the original while GUI runs
    CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/binnacle"
    mkdir -p "$CACHE_DIR"
    cp ~/.local/bin/bn "$CACHE_DIR/bn-gui"
    "$CACHE_DIR/bn-gui" gui&
    echo "Launched binnacle GUI at http://127.0.0.1:$BN_GUI_PORT (PID $!)"

# Run clippy with strict warnings
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
    cargo test --all-features
