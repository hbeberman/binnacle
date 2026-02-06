#!/usr/bin/env bash
# bootstrap.sh — Full binnacle development environment setup for Azure Linux 3
# Run this on a fresh Azure Linux 3 VM to get a working binnacle dev environment.
set -e

###############################################################################
# Configuration — change these as needed
###############################################################################
BINNACLE_BRANCH="hbeberman/02-04-26"
BINNACLE_REPO="https://github.com/hbeberman/binnacle.git"
REPOS_DIR="$HOME/repos"

###############################################################################
# 1. Install Rust toolchain
###############################################################################
echo "========================================"
echo "  1/6  Installing Rust toolchain"
echo "========================================"

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"

rustup target add wasm32-unknown-unknown
cargo install wasm-pack

###############################################################################
# 2. Install system packages
###############################################################################
echo "========================================"
echo "  2/6  Installing system packages"
echo "========================================"

sudo dnf install -y azurelinux-repos-extended
sudo dnf install -y \
    gcc make pkg-config openssl-devel \
    containerd buildah \
    nodejs npm git \
    glibc-devel kernel-headers binutils \
    netavark jq

sudo systemctl enable --now containerd

###############################################################################
# 3. Install npm packages
###############################################################################
echo "========================================"
echo "  3/6  Installing npm packages"
echo "========================================"

npm install -g marked highlight.js

###############################################################################
# 4. Setup rootless containerd
###############################################################################
echo "========================================"
echo "  4/6  Setting up rootless containerd"
echo "========================================"

sudo dnf install -y golang slirp4netns systemd-container

# Build rootlesskit from source
mkdir -p "$REPOS_DIR"
git clone https://github.com/rootless-containers/rootlesskit.git "$REPOS_DIR/rootlesskit"
pushd "$REPOS_DIR/rootlesskit"
make && sudo make install
popd

# Build passt/pasta (usermode networking)
git clone https://passt.top/passt "$REPOS_DIR/passt"
pushd "$REPOS_DIR/passt"
make && sudo make install
popd

# Configure subuid/subgid
sudo sh -c "echo \"$USER:100000:65536\" >> /etc/subuid"
sudo sh -c "echo \"$USER:100000:65536\" >> /etc/subgid"

# Install containerd-rootless.sh
git clone https://github.com/containerd/nerdctl.git "$REPOS_DIR/nerdctl"
sudo cp "$REPOS_DIR/nerdctl/extras/rootless/containerd-rootless.sh" /usr/local/bin/

# Setup user dbus socket
mkdir -p ~/.config/systemd/user

cat > ~/.config/systemd/user/dbus.socket <<'EOF'
[Unit]
Description=D-Bus User Message Bus Socket

[Socket]
ListenStream=%t/bus
ExecStartPost=-/usr/bin/systemctl --user set-environment DBUS_SESSION_BUS_ADDRESS=unix:path=%t/bus

[Install]
WantedBy=sockets.target
EOF

cat > ~/.config/systemd/user/dbus.service <<'EOF'
[Unit]
Description=D-Bus User Message Bus
Requires=dbus.socket

[Service]
ExecStart=/usr/bin/dbus-daemon --session --address=unix:path=%t/bus --nofork --nopidfile --systemd-activation
ExecReload=/usr/bin/dbus-send --print-reply --session --type=method_call --dest=org.freedesktop.DBus / org.freedesktop.DBus.ReloadConfig

[Install]
Also=dbus.socket
EOF

systemctl --user daemon-reload
systemctl --user enable --now dbus.socket

# Install and enable rootless containerd
"$REPOS_DIR/nerdctl/extras/rootless/containerd-rootless-setuptool.sh" install
systemctl --user enable --now containerd.service

###############################################################################
# 5. Clone and build binnacle
###############################################################################
echo "========================================"
echo "  5/6  Cloning and building binnacle"
echo "========================================"

# Add ~/.local/bin to PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
export PATH="$HOME/.local/bin:$PATH"

mkdir -p "$REPOS_DIR"
git clone -b "$BINNACLE_BRANCH" "$BINNACLE_REPO" "$REPOS_DIR/binnacle"
pushd "$REPOS_DIR/binnacle"
cargo install --path . --all-features

bn system host-init
bn system session-init
bn container build worker
popd

###############################################################################
# 6. PAT setup instructions
###############################################################################
echo "========================================"
echo "  6/6  Setup complete!"
echo "========================================"
echo ""
echo "To finish, create a fine-grained GitHub PAT:"
echo ""
echo "  1. Go to https://github.com/settings/tokens"
echo "  2. Click 'Generate new token' -> 'Fine-grained token'"
echo "  3. Name it (e.g., 'binnacle'), set expiration"
echo "  4. Repository access: select your target repos or 'All repositories'"
echo "  5. Permissions: enable 'Copilot Requests' -> Read-only"
echo "  6. Click 'Generate token' and copy it"
echo ""
echo "Then export it in your shell:"
echo ""
echo "  export COPILOT_GITHUB_TOKEN=<your-pat>"
echo ""
echo "Start using binnacle:"
echo ""
echo "  cd ~/repos/binnacle"
echo "  bn-agent buddy"
echo ""
