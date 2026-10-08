use bluer::gatt::{
    CharacteristicWriter,
    local::{
        CharacteristicControl,
        CharacteristicControlEvent,
        CharacteristicReadRequest,
        CharacteristicWriteRequest,
        ReqResult,
        ReqError,
    },
};

use futures::{StreamExt, pin_mut};
use std::{sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::sleep};
use nmrs::{NetworkManager, raw::zbus};
use serde::Serialize;

use crate::helper::parse_rssi_dbm;
use crate::scan;
use crate::setting::Command;
use crate::wifi;

/// Serializes the NM connect attempts started by `write_command`. The lock is held by the
/// background task for the whole attempt, so a `SetWifi` arriving meanwhile is rejected
/// instead of racing the one in flight.
pub type ConnectLock = Arc<Mutex<()>>;

#[derive(Debug, Serialize)]
struct NetworkStatus {
    connected: bool,
    ssid: Option<String>,
    signal: Option<i32>, // RSSI in dBm, e.g. -50
    ip: Option<String>,
}

/// Read the interface's current RSSI (dBm) from /proc/net/wireless
async fn read_rssi_dbm(iface: &str) -> Option<i32> {
    let text = tokio::fs::read_to_string("/proc/net/wireless").await.ok()?;
    parse_rssi_dbm(&text, iface)
}

pub async fn read_network_status(
    nm: NetworkManager,
    req: CharacteristicReadRequest
) -> ReqResult<Vec<u8>> {
    println!("NetworkStatus read from {} (mtu={}, offset={})",
             req.device_address, req.mtu, req.offset);

    let status = match nm.current_network().await {
        Ok(Some(net)) => NetworkStatus {
            connected: true,
            signal: read_rssi_dbm(&net.device).await,
            ssid: Some(net.ssid),
            ip: net.ip4_address.map(|ip| match ip.split_once('/') {
                Some((addr, _prefix)) => addr.to_string(),
                None => ip,
            }),
        },
        Ok(None) => NetworkStatus {
            connected: false,
            ssid: None,
            signal: None,
            ip: None,
        },  
        Err(e) => {
            println!("Failed to get current network status: {e}");
            return Err(ReqError::Failed);
        }
    };

    println!("status: {:?}", status);
    serde_json::to_vec(&status).map_err(|_| ReqError::Failed)
}

/// How long to wait after each round before rescanning
const RESCAN_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Serialize)]
struct AvailableNetwork {
    ssid: String,
    signal: Option<u8>, // NM strength, 0-100 (%)
    secured: bool,
}


pub async fn serve_available_networks(nm: NetworkManager, control: CharacteristicControl) {
    pin_mut!(control);
    while let Some(event) = control.next().await {
        match event {
            CharacteristicControlEvent::Notify(writer) => {
                tokio::spawn(notify_available_networks(nm.clone(), writer));
            }
            CharacteristicControlEvent::Write(req) => req.reject(ReqError::NotSupported),
        }
    }
}

async fn notify_available_networks(nm: NetworkManager, writer: CharacteristicWriter) {
    // IO mode exposes the MTU negotiated with this client, and bluer's send()
    // accepts at most mtu() bytes per call, so use it as the chunk size
    let chunk_len = writer.mtu();
    println!("Notification session start from {} (mtu={chunk_len})", writer.device_address());

    let conn = match zbus::Connection::system().await {
        Ok(conn) => conn,
        Err(e) => {
            println!("Failed to connect to system D-Bus: {e}");
            return;
        }
    };

    // Treat an is_closed() error as the connection having ended
    while !writer.is_closed().unwrap_or(true) {
        let networks = match scan::rescan(&nm, &conn).await {
            Ok(networks) => networks,
            Err(e) => {
                println!("Failed to scan networks: {e}");
                sleep(RESCAN_INTERVAL).await;
                continue;
            }
        };

        let list: Vec<AvailableNetwork> = networks
            .into_iter()
            .map(|n| AvailableNetwork { ssid: n.ssid, signal: n.strength, secured: n.secured })
            .collect();
        let mut payload = match serde_json::to_vec(&list) {
            Ok(payload) => payload,
            Err(e) => {
                println!("Failed to serialize networks: {e}");
                break;
            }
        };
        payload.push(b'\n');

        for chunk in payload.chunks(chunk_len) {
            if let Err(e) = writer.send(chunk).await {
                println!("Notification session ended: {e}");
                return;
            }
        }
        println!("Notified {} networks ({} bytes, chunk={chunk_len})", list.len(), payload.len());

        sleep(RESCAN_INTERVAL).await;
    }
    println!("Notification session stopped by client");
}

/// Write callback for the Command characteristic.
///
/// Parses and validates the Command, returning an error to the phone if it's malformed.
/// Slow operations (connecting can take up to 30 s) go to a background task and this returns
/// Ok immediately, to stay within the 30 s ATT timeout; the phone polls `NetworkStatus` for the
/// outcome.
pub async fn write_command(
    nm: NetworkManager,
    connect_lock: ConnectLock,
    value: Vec<u8>,
    req: CharacteristicWriteRequest,
) -> ReqResult<()> {
    println!("Command write from {} ({} bytes, mtu={}, offset={})",
             req.device_address, value.len(), req.mtu, req.offset);

    // Long writes (Prepare Write) aren't supported yet; the whole Command must arrive in one write
    if req.offset != 0 {
        return Err(ReqError::InvalidOffset);
    }

    let cmd = Command::parse(&value).map_err(|e| {
        println!("Invalid command: {e:?}");
        ReqError::from(e)
    })?;

    match cmd {
        Command::SetWifi { id, ssid, password } => {
            // The lock is released when the spawned task ends, i.e. after NM finished
            let Ok(guard) = connect_lock.try_lock_owned() else {
                println!("[{id}] SetWifi rejected: a connect attempt is still running");
                return Err(ReqError::InProgress);
            };

            println!("[{id}] SetWifi received: ssid=\"{ssid}\", connecting in background");
            tokio::spawn(async move {
                match wifi::switch_network(&nm, &ssid, &password).await {
                    Ok(()) => println!("[{id}] Connected to \"{ssid}\""),
                    Err(e) => println!("[{id}] Failed to connect to \"{ssid}\": {e}"),
                }
                drop(guard);
            });
            Ok(())
        }
        Command::Disconnect { id } => {
            println!("[{id}] Disconnect: not implemented yet");
            Err(ReqError::NotSupported)
        }
    }
}
