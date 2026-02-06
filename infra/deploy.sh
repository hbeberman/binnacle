#!/usr/bin/env bash
# deploy.sh — Deploy binnacle dev VM and print connection info
set -euo pipefail

RG="${RG:-hebeberm-binnacle-rg}"
USERNAME="${1:-henry}"
SSH_KEY="${SSH_KEY:-$(cat ~/.ssh/id_rsa.pub)}"

echo "Deploying binnacle VM for ${USERNAME} in ${RG}..."
result=$(az deployment group create \
  -g "$RG" \
  -f "$(dirname "$0")/main.bicep" \
  -p username="$USERNAME" sshPublicKey="$SSH_KEY" \
  --query "properties.outputs" \
  -o json)

ip=$(echo "$result" | jq -r '.publicIpAddress.value')
ssh_cmd=$(echo "$result" | jq -r '.sshCommand.value')

echo ""
echo "========================================="
echo "  VM deployed successfully!"
echo "  IP Address: ${ip}"
echo "  Connect:    ${ssh_cmd}"
echo "========================================="
