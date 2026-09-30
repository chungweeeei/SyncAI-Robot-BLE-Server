mod config;
mod setting;
mod handler;
mod helper;
mod scan;
mod wifi;

use bluer::{
    adv::Advertisement,
    gatt::local::{
        Application, Characteristic, CharacteristicNotify, CharacteristicNotifyMethod,
        CharacteristicRead, CharacteristicWrite, CharacteristicWriteMethod, Service,
        characteristic_control,
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
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    env_logger::init();

    // network manager session
    let nm = nmrs::NetworkManager::new().await?;
    let nm_for_read = nm.clone();
    let nm_for_notify = nm.clone();
    let nm_for_command = nm.clone();

    // bluetooth bluer session
    let session= bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;

    println!("Advertising on Bluetooth adapter {} with address {}", adapter.name(), adapter.address().await?);
    let adv = Advertisement {
        service_uuids: vec![PRIMARY_SERVICE_UUID].into_iter().collect(),
        discoverable: Some(true),
        local_name: Some(String::from("test")),
        ..Default::default()
    };
    let adv_handle = adapter.advertise(adv).await?;
    println!("Serving GATT service on Bluetooth adapter {}", adapter.name());


    // AvailableNetworks uses IO mode: the control end receives subscription events,
    // the handle end is given to the Characteristic
    let (networks_control, networks_handle) = characteristic_control();

    let app = Application {
        services: vec![Service {
            uuid: PRIMARY_SERVICE_UUID,
            primary: true,
            characteristics: vec![Characteristic {
                uuid: ServiceCharacteristicUUID::NetworkStatus.uuid(),
                read: Some(CharacteristicRead {
                    read: true,
                    fun: Box::new(move |req| handler::read_network_status(nm_for_read.clone(), req).boxed()),
                    ..Default::default()
                }),
                ..Default::default()
            }, Characteristic {
                uuid: ServiceCharacteristicUUID::Command.uuid(),
                write: Some(CharacteristicWrite {
                    write: true,
                    method: CharacteristicWriteMethod::Fun(Box::new(move |value, req| {
                        handler::write_command(nm_for_command.clone(), value, req).boxed()
                    })),
                    ..Default::default()
                }),
                ..Default::default()
            }, Characteristic {
                uuid: ServiceCharacteristicUUID::AvailableNetworks.uuid(),
                notify: Some(CharacteristicNotify {
                    notify: true,
                    method: CharacteristicNotifyMethod::Io,
                    ..Default::default()
                }),
                control_handle: networks_handle,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let app_handle = adapter.serve_gatt_application(app).await?;
    tokio::spawn(handler::serve_available_networks(nm_for_notify, networks_control));
    println!("Service ready. Press enter to quit");
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    let _ = lines.next_line().await?;

    println!("Removing service and advertisement");
    drop(app_handle);
    drop(adv_handle);
    sleep(Duration::from_secs(1)).await;

    Ok(())
}   
