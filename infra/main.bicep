// main.bicep — Azure Linux 3 VM with networking and cloud-init bootstrap delivery
// Deploys a dev VM for binnacle development with rootless containerd support.

@description('Team member username (used for resource naming and VM admin)')
param username string

@description('SSH public key content for authentication')
param sshPublicKey string

@description('Azure region for all resources')
param location string = 'southcentralus'

@description('VM size SKU')
param vmSize string = 'Standard_D16ds_v5'

@description('OS disk size in GB')
@minValue(30)
param osDiskSizeGB int = 100

// Resource naming convention: {username}-binnacle-*
var vmName = '${username}-binnacle-vm'
var vnetName = '${username}-binnacle-vnet'
var subnetName = 'default'
var nsgName = '${username}-binnacle-nsg'
var pipName = '${username}-binnacle-pip'
var nicName = '${username}-binnacle-nic'

// Embed bootstrap.sh content via loadTextContent() for cloud-init delivery
var bootstrapScript = loadTextContent('bootstrap.sh')

// Cloud-init YAML that writes bootstrap.sh to the user's home directory
var cloudInit = '#cloud-config\nwrite_files:\n  - path: /home/${username}/bootstrap.sh\n    permissions: \'0755\'\n    owner: ${username}:${username}\n    content: |\n${indent(bootstrapScript, 6)}\n'

// Network Security Group — allow SSH inbound
resource nsg 'Microsoft.Network/networkSecurityGroups@2024-01-01' = {
  name: nsgName
  location: location
  properties: {
    securityRules: [
      {
        name: 'AllowSSH'
        properties: {
          priority: 1000
          direction: 'Inbound'
          access: 'Allow'
          protocol: 'Tcp'
          sourceAddressPrefix: '*'
          sourcePortRange: '*'
          destinationAddressPrefix: '*'
          destinationPortRange: '22'
        }
      }
    ]
  }
}

// Virtual Network
resource vnet 'Microsoft.Network/virtualNetworks@2024-01-01' = {
  name: vnetName
  location: location
  properties: {
    addressSpace: {
      addressPrefixes: [
        '10.0.0.0/16'
      ]
    }
    subnets: [
      {
        name: subnetName
        properties: {
          addressPrefix: '10.0.0.0/24'
          networkSecurityGroup: {
            id: nsg.id
          }
        }
      }
    ]
  }
}

// Public IP — static allocation
resource pip 'Microsoft.Network/publicIPAddresses@2024-01-01' = {
  name: pipName
  location: location
  sku: {
    name: 'Standard'
  }
  properties: {
    publicIPAllocationMethod: 'Static'
  }
}

// Network Interface
resource nic 'Microsoft.Network/networkInterfaces@2024-01-01' = {
  name: nicName
  location: location
  properties: {
    ipConfigurations: [
      {
        name: 'ipconfig1'
        properties: {
          privateIPAllocationMethod: 'Dynamic'
          publicIPAddress: {
            id: pip.id
          }
          subnet: {
            id: vnet.properties.subnets[0].id
          }
        }
      }
    ]
  }
}

// Virtual Machine — Azure Linux 3 Gen2, SSH key auth only
resource vm 'Microsoft.Compute/virtualMachines@2024-07-01' = {
  name: vmName
  location: location
  properties: {
    hardwareProfile: {
      vmSize: vmSize
    }
    osProfile: {
      computerName: vmName
      adminUsername: username
      customData: base64(cloudInit)
      linuxConfiguration: {
        disablePasswordAuthentication: true
        ssh: {
          publicKeys: [
            {
              path: '/home/${username}/.ssh/authorized_keys'
              keyData: sshPublicKey
            }
          ]
        }
      }
    }
    storageProfile: {
      imageReference: {
        publisher: 'MicrosoftCBLMariner'
        offer: 'azure-linux-3'
        sku: 'azure-linux-3-gen2'
        version: 'latest'
      }
      osDisk: {
        name: '${vmName}-osdisk'
        createOption: 'FromImage'
        diskSizeGB: osDiskSizeGB
        managedDisk: {
          storageAccountType: 'Premium_LRS'
        }
      }
    }
    networkProfile: {
      networkInterfaces: [
        {
          id: nic.id
        }
      ]
    }
  }
}

// Outputs
output vmName string = vm.name
output publicIpAddress string = pip.properties.ipAddress
output sshCommand string = 'ssh ${username}@${pip.properties.ipAddress}'
