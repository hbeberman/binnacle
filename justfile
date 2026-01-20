# Binnacle build commands

# Build the project (debug or release)
build mode="debug":
    @if [ "{{mode}}" = "release" ]; then \
        cargo build --release; \
    else \
        cargo build; \
    fi

# Install release build to ~/.local/bin
install: (build "release")
    mkdir -p ~/.local/bin
    cp target/release/bn ~/.local/bin/
    @echo "Installed bn to ~/.local/bin/bn"
