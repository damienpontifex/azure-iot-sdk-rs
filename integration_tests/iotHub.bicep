param location string
param environmentName string
param tags object

resource iothub 'Microsoft.Devices/IotHubs@2020-03-01' = {
  name: environmentName
  location: location
  tags: tags
  sku: {
    name: 'B1'
    capacity: 1
  }
  properties: {}
}

