#!/usr/bin/env bash
# bootstrap.sh — Full binnacle development environment setup for Azure Linux 3
# Run this on a fresh Azure Linux 3 VM to get a working binnacle dev environment.
# Requires TWO runs: first run configures subuid/subgid and forces re-login,
# second run completes the full setup.
#
# Usage: bootstrap.sh [--phase N]
#   --phase N   Skip to phase N (0-7). Useful for resuming after a failure.
set -e

###############################################################################
# Configuration — change these as needed
###############################################################################
BINNACLE_BRANCH="hbeberman/02-04-26"
BINNACLE_REPO="https://github.com/hbeberman/binnacle.git"
REPOS_DIR="$HOME/repos"

START_PHASE=0
if [[ "$1" == "--phase" && -n "$2" ]]; then
  START_PHASE="$2"
  shift 2
fi

banner() {
  echo ""
  echo "========================================"
  echo "  $1"
  echo "========================================"
  echo ""
}

phase_at_least() {
  [[ "$START_PHASE" -le "$1" ]]
}

###############################################################################
# Phase gate: subuid/subgid must be configured before anything else
###############################################################################
if ! grep -q "^$USER:" /etc/subuid 2>/dev/null; then
  banner "First run — configuring rootless container support"

  echo "  Rootless containers (buildah, containerd) require subuid/subgid"
  echo "  mappings in /etc/subuid and /etc/subgid. These mappings only take"
  echo "  effect after a fresh login, so this first run will:"
  echo ""
  echo "    1. Add $USER to /etc/subuid and /etc/subgid"
  echo "    2. Set newuidmap/newgidmap permissions"
  echo "    3. Force logout all of your sessions"
  echo ""
  echo "  After logging back in, run bootstrap.sh again to continue setup."
  echo ""
  read -rp "  Proceed? (Y/n): " answer
  if [[ "$answer" =~ ^[Nn] ]]; then
    echo "  Aborted."
    exit 0
  fi

  echo ""
  echo "  → Adding $USER to /etc/subuid and /etc/subgid..."
  sudo sh -c "echo \"$USER:100000:65536\" >> /etc/subuid"
  sudo sh -c "echo \"$USER:100000:65536\" >> /etc/subgid"

  echo "  → Setting newuidmap/newgidmap setuid permissions..."
  sudo chmod u+s /usr/bin/newuidmap /usr/bin/newgidmap

  echo "  → Adding ~/.local/bin and ~/.cargo/bin to PATH..."
  echo 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"' >> ~/.bashrc

  echo "  → Setting default git identity and branch name..."
  git config --global user.name "binnacle-bot"
  git config --global user.email "noreply@binnacle.bot"
  git config --global init.defaultBranch main

  echo ""
  echo "  ✅ subuid/subgid configured. Logging out all sessions now..."
  echo "  → SSH back in and run: bootstrap.sh"
  echo ""

  loginctl terminate-user "$USER"
  exit 0
fi

###############################################################################
# 1. Install all system packages upfront
###############################################################################
if phase_at_least 1; then
  banner "1/7  Installing system packages (dnf)"

  echo "  → Enabling extended repos..."
  sudo dnf install -y azurelinux-repos-extended

  echo "  → Installing build tools, container runtimes, and dev dependencies..."
  sudo dnf install -y \
      gcc make pkg-config openssl-devel \
      containerd buildah \
      nodejs npm git \
      glibc-devel kernel-headers binutils \
      netavark jq \
      golang slirp4netns systemd-container

  echo "  → Enabling containerd service..."
  sudo systemctl enable --now containerd
fi

###############################################################################
# 2. Install Rust toolchain
###############################################################################
if phase_at_least 2; then
  banner "2/7  Installing Rust toolchain"

  echo "  → Installing rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"

  echo "  → Adding wasm32 target and wasm-pack..."
  rustup target add wasm32-unknown-unknown
  cargo install wasm-pack
fi

###############################################################################
# 3. Install npm packages
###############################################################################
if phase_at_least 3; then
  banner "3/7  Configuring npm and installing global packages"

  echo "  → Setting npm prefix to ~/.local..."
  npm config set prefix ~/.local

  echo "  → Installing marked and highlight.js..."
  npm install -g marked highlight.js
fi

###############################################################################
# 4. Setup rootless containerd
###############################################################################
if phase_at_least 4; then
  banner "4/7  Setting up rootless containerd"

  echo "  → Building rootlesskit from source..."
  mkdir -p "$REPOS_DIR"
  git clone https://github.com/rootless-containers/rootlesskit.git "$REPOS_DIR/rootlesskit"
  pushd "$REPOS_DIR/rootlesskit"
  make && sudo make install
  popd

  echo "  → Building passt/pasta (usermode networking)..."
  git clone https://passt.top/passt "$REPOS_DIR/passt"
  pushd "$REPOS_DIR/passt"
  make && sudo make install
  popd

  echo "  → Installing containerd-rootless.sh..."
  git clone https://github.com/containerd/nerdctl.git "$REPOS_DIR/nerdctl"
  sudo cp "$REPOS_DIR/nerdctl/extras/rootless/containerd-rootless.sh" /usr/local/bin/

  echo "  → Setting up user dbus socket..."
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

  echo "  → Installing and enabling rootless containerd..."
  "$REPOS_DIR/nerdctl/extras/rootless/containerd-rootless-setuptool.sh" install
  systemctl --user enable --now containerd.service
fi

###############################################################################
# 5. Clone and build binnacle
###############################################################################
if phase_at_least 5; then
  banner "5/7  Cloning and building binnacle"

  echo "  → Cloning binnacle (branch: $BINNACLE_BRANCH)..."
  mkdir -p "$REPOS_DIR"
  git clone -b "$BINNACLE_BRANCH" "$BINNACLE_REPO" "$REPOS_DIR/binnacle"
  pushd "$REPOS_DIR/binnacle"

  echo "  → Installing npm dependencies for web bundle..."
  npm install

  echo "  → Building binnacle (release mode, this may take a few minutes)..."
  cargo install --path . --all-features
  popd
fi

###############################################################################
# 6. Initialize binnacle and set up starter project
###############################################################################
if phase_at_least 6; then
  banner "6/7  Initializing binnacle system"

  echo "  → Running bn system host-init..."
  echo "    This will install Copilot CLI, set up agent scripts,"
  echo "    and build the binnacle container image."
  echo ""
  pushd "$REPOS_DIR/binnacle"
  bn system host-init
  popd

  banner "6/7  Setting up starter project"

  echo "  → Creating ~/repos/project as a ready-to-use binnacle workspace..."
  mkdir -p "$REPOS_DIR/project"
  pushd "$REPOS_DIR/project"
  git init
  cp -r "$REPOS_DIR/binnacle/.binnacle" .

  cat > README.md <<'EOF'
# My Project

This project is managed with [binnacle](https://github.com/hbeberman/binnacle) — a task and workflow tracker for AI-assisted development.

## Quick Start

```bash
# Set your GitHub PAT for Copilot access
export COPILOT_GITHUB_TOKEN=<your-pat>

# Launch an agent
bn-agent buddy
```

## Useful Commands

- `bn ready` — Show tasks ready to work on
- `bn task create "Title"` — Create a new task
- `bn orient` — Get project overview
- `bn-agent buddy` — Launch an AI agent session
EOF

  git add .
  git commit -m "Initial commit: binnacle project setup"

  echo "  → Initializing binnacle session..."
  bn session init
  popd
fi

###############################################################################
# 7. Done!
###############################################################################
banner "7/7  Setup complete!"

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
echo "To persist across sessions (⚠️  stored in plaintext — only use on"
echo "trusted VMs with restricted access):"
echo ""
echo "  echo 'export COPILOT_GITHUB_TOKEN=<your-pat>' >> ~/.bashrc"
echo ""
echo "Start using binnacle:"
echo ""
echo "  cd ~/repos/project"
echo "  bn-agent buddy"
echo ""
