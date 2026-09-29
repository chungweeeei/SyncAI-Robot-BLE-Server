mod config;
mod setting;

use bluer::{
    adv::Advertisement,
    gatt::local::{
        Application, Characteristic, CharacteristicNotify, CharacteristicNotifyMethod,
        CharacteristicRead, CharacteristicWrite, CharacteristicWriteMethod, Service,
    },
};
use futures::FutureExt;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::Mutex,
    time::sleep,
};

use config::{PRIMARY_SERVICE_UUID, ServiceCharacteristicUUID};


#[tokio::main(flavor="current_thread")]
async fn main() -> bluer::Result<()>{
    env_logger::init();

    // create bluer session
    let session= bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;

    println!("Advertising on Bluetooth adapter {} with address {}", adapter.name(), adapter.address().await?);
    let adv = Advertisement {
        service_uuids: vec![PRIMARY_SERVICE_UUID].into_iter().collect(),
        discoverable: Some(true),
        local_name: Some(String::from("SyncAI-Test-Robot")),
        ..Default::default()
    };
    let adv_handle = adapter.advertise(adv).await?;
    println!("Serving GATT service on Bluetooth adapter {}", adapter.name());


    let app = Application {
        services: vec![Service {
            uuid: PRIMARY_SERVICE_UUID,
            primary: true,
            characteristics: vec![Characteristic {
                uuid: ServiceCharacteristicUUID::NetworkStatus.uuid(),
                read: Some(CharacteristicRead {
                    read: true,
                    let 
                }),
            }],
        }]
    }


    Ok(())
}   
