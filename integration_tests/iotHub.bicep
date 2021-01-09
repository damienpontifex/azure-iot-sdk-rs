param location string
param environmentName string
param tags object

var iothubKeyName = 'iothubowner'

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

resource dps 'Microsoft.Devices/provisioningServices@2020-03-01' = {
  name: environmentName
  location: location
  tags: tags
  sku: {
    name: 'S1'
    capacity: 1
  }
  properties: {
    iotHubs: [
      {
        location: location
        connectionString: 'HostName=${iothub.properties.hostName};SharedAccessKeyName=${iothubKeyName};SharedAccessKey=${listKeys(resourceId('Microsoft.Devices/Iothubs/Iothubkeys', iothub.name, iothubKeyName), iothub.apiVersion).primaryKey}'
      }
    ]
  }
}

resource budget 'Microsoft.Consumption/budgets@2019-10-01' = {
  name: 'BudgetLimit'
  properties: {
    amount: 20
    timeGrain: 'BillingMonth'
    category: 'Cost'
    timePeriod: {
      startDate: '2020-12-01'
    }
    notifications: {
      actual_GreaterThan_80_Percent: {
        enabled: true
        contactEmails: [
          'damien.pontifex@gmail.com'
        ]
        threshold: 80
        operator: 'GreaterThan'
      }
    }
  }
}