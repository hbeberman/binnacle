# Binnacle Container Worker
# Base: Fedora 43
FROM docker.io/library/fedora:43

LABEL org.opencontainers.image.title="binnacle-worker"
LABEL org.opencontainers.image.description="AI agent worker with binnacle task tracking"
LABEL org.opencontainers.image.source="https://github.com/henry/binnacle"

# Install system dependencies (including build tools for Rust compilation)
RUN dnf install -y \
    git \
    curl \
    wget \
    jq \
    ripgrep \
    fd-find \
    tree \
    vim \
    nodejs \
    npm \
    python3 \
    python3-pip \
    gcc \
    make \
    openssl-devel \
    pkg-config \
    && dnf clean all

# Install Rust (for building binnacle)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install binnacle from source
COPY . /build/binnacle
WORKDIR /build/binnacle
RUN cargo build --release && \
    cp target/release/bn /usr/local/bin/bn && \
    rm -rf /build

# Install GitHub Copilot CLI
# Note: Requires COPILOT_GITHUB_TOKEN at runtime for auth
RUN npm install -g @anthropic-ai/claude-cli || true

# Create working directories
RUN mkdir -p /workspace /binnacle /copilot-config

# Set container mode environment variables
ENV BN_CONTAINER_MODE=true
ENV BN_AGENT_TYPE=worker
ENV BN_MERGE_TARGET=main

# Copy entrypoint script
COPY container/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

WORKDIR /workspace
ENTRYPOINT ["/entrypoint.sh"]
