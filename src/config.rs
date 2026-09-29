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
            self::Command => Uuid::from_u128(0x1234abcd-0000-0000-8000-00805f9b34fb),
            self::NetworkStatus => Uuid::from_u128(0x1234abcd-0001-0000-8000-00805f9b34fb),
            self::AvailableNetworks => Uuid::from_u128(0x1234abcd-0002-0000-8000-00805f9b34fb),
        }
    }
}
