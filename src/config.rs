use bluer::Uuid;

pub const PRIMARY_SERVICE_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_56789abcdef0);

pub enum ServiceCharacteristicUUID {
    Command,
    NetworkStatus,
    AvailableNetworks,
}

impl ServiceCharacteristicUUID {
    pub const fn uuid(&self) -> Uuid {
        match self {
            ServiceCharacteristicUUID::Command => Uuid::from_u128(0x1234abcd_0000_0000_8000_00805f9b34fb),
            ServiceCharacteristicUUID::NetworkStatus => Uuid::from_u128(0x1234abcd_0001_0000_8000_00805f9b34fb),
            ServiceCharacteristicUUID::AvailableNetworks => Uuid::from_u128(0x1234abcd_0002_0000_8000_00805f9b34fb),
        }
    }
}
