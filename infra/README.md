# Binnacle Infrastructure

Azure VM provisioning for binnacle development environments using Bicep and cloud-init.

## Overview

This directory contains infrastructure-as-code for one-command Azure VM provisioning. Team members deploy a Bicep template, SSH into the VM, run `~/bootstrap.sh`, and have a fully working binnacle development environment with rootless containerd.

## File Layout

```
infra/
├── main.bicep          # Azure VM + networking resources (Bicep template)
├── main.bicepparam     # Default parameter values
├── bootstrap.sh        # Dev environment setup script (placed in ~ via cloud-init)
└── README.md           # This file
```

## Prerequisites

- [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/install-azure-cli) (`az`) installed locally
- Access to the shared Azure subscription
- SSH key pair (`~/.ssh/id_rsa.pub`)
- GitHub access for cloning the binnacle repo

## Quick Start

### 1. Create a resource group

```bash
az group create -g <username>-binnacle-rg -l southcentralus
```

### 2. Deploy the VM

```bash
az deployment group create \
  -g <username>-binnacle-rg \
  -f infra/main.bicep \
  -p username=<username> \
     sshPublicKey="$(cat ~/.ssh/id_rsa.pub)"
```

Alternatively, edit `main.bicepparam` with your values and deploy using the parameter file:

```bash
az deployment group create \
  -g <username>-binnacle-rg \
  -f infra/main.bicep \
  -p infra/main.bicepparam
```

The deployment takes a few minutes. On success, the output includes:

- `vmName` — the created VM's name
- `publicIpAddress` — the VM's public IP
- `sshCommand` — a ready-to-use SSH command

### 3. SSH into the VM

```bash
ssh <username>@<public-ip>
```

Use the `sshCommand` from the deployment output, or find the IP with:

```bash
az deployment group show \
  -g <username>-binnacle-rg \
  -n main \
  --query properties.outputs.publicIpAddress.value -o tsv
```

### 4. Run the bootstrap script

```bash
~/bootstrap.sh
```

The script is placed in your home directory by cloud-init during VM provisioning. It runs through six stages:

1. **Rust toolchain** — rustup, wasm32 target, wasm-pack
2. **System packages** — gcc, openssl-devel, containerd, buildah, nodejs, npm, git
3. **npm packages** — marked, highlight.js
4. **Rootless containerd** — rootlesskit, passt/pasta, nerdctl, dbus user socket, subuid/subgid
5. **Binnacle** — clone, build, `bn system host-init`, `bn system session-init`, `bn container build worker`
6. **PAT instructions** — printed at the end

Takes approximately 20–30 minutes (mostly unattended).

### 5. Set up your GitHub PAT

After bootstrap completes, create a fine-grained GitHub PAT:

1. Go to <https://github.com/settings/tokens>
2. Click **Generate new token** → **Fine-grained token**
3. Set a name (e.g., `binnacle`) and expiration
4. Repository access: select your target repos or **All repositories**
5. Permissions: enable **Copilot Requests → Read-only**
6. Click **Generate token** and copy it

Then export it in your shell:

```bash
export COPILOT_GITHUB_TOKEN=<your-pat>
```

### 6. Start using binnacle

```bash
cd ~/repos/binnacle
bn-agent buddy
```

## Bicep Template Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `username` | string | *(required)* | Team member's username (resource naming + VM admin) |
| `sshPublicKey` | string | *(required)* | SSH public key content for authentication |
| `location` | string | `southcentralus` | Azure region |
| `vmSize` | string | `Standard_D16ds_v5` | VM size SKU |
| `osDiskSizeGB` | int | `100` | OS disk size in GB |

## Teardown

To delete all resources when you're done:

```bash
az group delete -g <username>-binnacle-rg --yes --no-wait
```

## Configuration

The bootstrap script clones binnacle from a specific branch. To change it, edit the `BINNACLE_BRANCH` variable at the top of `bootstrap.sh` before running:

```bash
vi ~/bootstrap.sh   # change BINNACLE_BRANCH="hbeberman/02-04-26" to your branch
~/bootstrap.sh
```

## Troubleshooting

- **SSH connection refused**: Wait a few minutes after deployment for the VM to finish booting and cloud-init to complete.
- **bootstrap.sh not found**: Cloud-init may still be running. Check with `cloud-init status --wait`.
- **bootstrap.sh permission denied**: Run `chmod +x ~/bootstrap.sh` and retry.
- **Build failures in bootstrap.sh**: Ensure the VM has internet access and the binnacle repo branch exists. Check `BINNACLE_BRANCH` at the top of the script.
- **DNS resolution issues in containers**: See the rootless containerd DNS fix in the binnacle docs.
- **Deployment fails with quota error**: The default VM size (`Standard_D16ds_v5`) requires sufficient vCPU quota. Request a quota increase or use a smaller `vmSize` parameter.
- **Re-running bootstrap.sh**: The script uses `git clone` which fails if the target directories already exist. Remove `~/repos/` before re-running: `rm -rf ~/repos && ~/bootstrap.sh`.
