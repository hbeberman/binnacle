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
    pkill bn || :
    just install
    # Copy to temp location so builds can replace the original while GUI runs
    CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/binnacle"
    mkdir -p "$CACHE_DIR"
    cp ~/.local/bin/bn "$CACHE_DIR/bn-gui"
    # Use fixed port (3030) for predictable testing - can be overridden with BN_GUI_PORT env var
    export BN_GUI_PORT="${BN_GUI_PORT:-3030}"
    "$CACHE_DIR/bn-gui" gui&
    echo "Launched binnacle GUI at http://127.0.0.1:$BN_GUI_PORT (PID $!)"

# Run clippy with strict warnings
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
    cargo test --all-features
