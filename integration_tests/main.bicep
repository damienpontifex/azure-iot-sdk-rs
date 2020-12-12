targetScope = 'subscription'

param location string = 'australiaeast'
param environmentName string = 'azure-iot-sdk-rs'

var tags = {
  Client: 'github.com/damienpontifex/azure-iot-sdk-rs'
  Environment: 'ci'
  ApplicationName: environmentName
}

resource rg 'Microsoft.Resources/resourceGroups@2020-06-01' = {
  name: environmentName
  location: location
  tags: tags
}

module iotHub './iotHub.bicep' = {
  name: 'iothub'
  scope: resourceGroup(rg.name)
  params: {
    environmentName: environmentName
    location: location
    tags: tags
  }
}