# AZL3 Rootless Containerd Bootstrap
## Binnacle Build Dependencies
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
. "$HOME/.cargo/env"

sudo dnf install -y azurelinux-repos-extended
sudo dnf install -y gcc make pkg-config openssl-devel containerd buildah nodejs npm git glibc-devel kernel-headers binutils netavark jq
sudo systemctl enable --now containerd

npm install marked highlight.js

echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
export PATH=$HOME/.local/bin/:$PATH
```

## Rootless Containerd Setup
> [!NOTE]
> You can skip Rootless Containerd if you allow binnacle access to passwordless sudo for ctr calls, enabled by setting BN_ALLOW_SUDO=1 when calling bn-agent.

```bash
sudo dnf install -y golang slirp4netns systemd-container
git clone https://github.com/rootless-containers/rootlesskit.git ~/repos/rootlesskit
pushd ~/repos/rootlesskit
make && sudo make install
popd

# Usermode Network  
git clone https://passt.top/passt
pushd passt
make && sudo make install
popd

sudo sh -c 'echo "$SUDO_USER:100000:65536" >> /etc/subuid'
sudo sh -c 'echo "$SUDO_USER:100000:65536" >> /etc/subgid'

git clone https://github.com/containerd/nerdctl.git ~/repos/nerdctl
sudo cp ~/repos/nerdctl/extras/rootless/containerd-rootless.sh /usr/local/bin/

# Setup a user dbus socket
mkdir -p ~/.config/systemd/user

# Setup dbus.socket
cat > ~/.config/systemd/user/dbus.socket <<'EOF'
   [Unit]
   Description=D-Bus User Message Bus Socket

   [Socket]
   ListenStream=%t/bus
   ExecStartPost=-/usr/bin/systemctl --user set-environment DBUS_SESSION_BUS_ADDRESS=unix:path=%t/bus

   [Install]
   WantedBy=sockets.target
EOF

# Setup dbus.service
cat > ~/.config/systemd/user/dbus.service <<'EOF'
   [Unit]
   Description=D-Bus User Message Bus
   Requires=dbus.socket

   [Service]
   ExecStart=/usr/bin/dbus-daemon --session --address=unix:path=%t/bus --nofork --nopidfile --systemd-activation
   ExecReload=/usr/bin/dbus-send --print-reply --session --type=method_call --dest=org.freedesktop.DBus /
  org.freedesktop.DBus.ReloadConfig

   [Install]
   Also=dbus.socket
EOF

# Enable and start user dbus
systemctl --user daemon-reload
systemctl --user enable --now dbus.socket

~/repos/nerdctl/extras/rootless/containerd-rootless-setuptool.sh install
systemctl --user enable --now containerd.service
```

## Grabbing a GH Copilot Enabled PAT
**Get a GitHub PAT with Copilot access:**
1. Go to https://github.com/settings/tokens
2. Click **"Generate new token"** → **"Fine-grained token"**
3. Name it (e.g., "binnacle"), set expiration (default is fine)
4. **Repository access**: select your target repos or "All repositories"
5. **Permissions**: enable **"Copilot Requests"** → Read-only
6. Click **"Generate token"** and copy it

## Install and run Binnacle!
```bash
git clone https://github.com/hbeberman/binnacle.git ~/repos/binnacle
pushd ~/reops/binnacle
cargo install --path . --all-features

# Initialize some system wide directories and configs for binnacle, including the default container
bn system host-init

# Initialize the current git directory as a binnacle session
bn system session-init

# Build the project specific worker container
bn container build worker

# Setup your PAT somewhere, for just a session or persist it, up to you.
export COPILOT_GITHUB_TOKEN=<PAT>

# Interact with the graph via an agent, handles simple tasks and bug reports well
bn-agent buddy

# Plan larger feature/investigations that split into multiple tasks
bn-agent prd

# Automatically work through open bugs and tasks until none are ready, then sleep.
bn-agent auto

# Chat with binnacle about binnacle
bn-agent qa

# Binnacle orientation without further guidance on task type
bn-agent free
```
