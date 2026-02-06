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

### 3. SSH into the VM

```bash
ssh <username>@<public-ip>
```

The public IP is shown in the deployment output.

### 4. Run the bootstrap script

```bash
~/bootstrap.sh
```

This installs the Rust toolchain, system packages, rootless containerd, and builds binnacle from source. Takes approximately 20–30 minutes (mostly unattended).

### 5. Set up your GitHub PAT

After bootstrap completes, follow the printed instructions to create a fine-grained GitHub PAT with **Copilot Requests: Read-only** permission, then:

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

## Troubleshooting

- **SSH connection refused**: Wait a few minutes after deployment for the VM to finish booting and cloud-init to complete.
- **bootstrap.sh not found**: Cloud-init may still be running. Check with `cloud-init status --wait`.
- **Build failures in bootstrap.sh**: Ensure the VM has internet access and the binnacle repo branch exists.
- **DNS resolution issues in containers**: See the rootless containerd DNS fix in the binnacle docs.
