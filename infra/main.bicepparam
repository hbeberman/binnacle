using 'main.bicep'

// Required — replace before deploying
param username = '<your-username>'
param sshPublicKey = '<your-ssh-public-key>'

// Defaults — override as needed
param location = 'southcentralus'
param vmSize = 'Standard_D16ds_v5'
param osDiskSizeGB = 100
