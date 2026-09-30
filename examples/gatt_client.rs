//! GATT client for testing. Run it on a *different* Linux machine with Bluetooth
//! (an adapter can't connect to itself).
//!
//! Usage:
//!   cargo run --example gatt_client -- status
//!   cargo run --example gatt_client -- scan [rounds, default 1]
//!   cargo run --example gatt_client -- set <ssid> [password]
//!   cargo run --example gatt_client -- disconnect
//!   cargo run --example gatt_client -- raw '<json>'     # send arbitrary data to test error handling
//!
//! By default it scans for a device advertising PRIMARY_SERVICE_UUID;
//! set BLE_ADDR=AA:BB:CC:DD:EE:FF to connect to a specific server.

use std::{env, time::Duration};

use bluer::{Adapter, AdapterEvent, Address, Device, Uuid, gatt::remote::Characteristic};
use futures::{StreamExt, pin_mut};
use tokio::time::timeout;

// Examples can't `use` modules from a binary crate (there's no lib.rs),
// so the UUIDs are copied here and must stay in sync with src/config.rs.
const PRIMARY_SERVICE_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_56789abcdef0);
const COMMAND_UUID: Uuid = Uuid::from_u128(0x1234abcd_0000_0000_8000_00805f9b34fb);
const NETWORK_STATUS_UUID: Uuid = Uuid::from_u128(0x1234abcd_0001_0000_8000_00805f9b34fb);
const AVAILABLE_NETWORKS_UUID: Uuid = Uuid::from_u128(0x1234abcd_0002_0000_8000_00805f9b34fb);

const DISCOVER_TIMEOUT: Duration = Duration::from_secs(20);
/// Each round the server waits for the Wi-Fi scan to finish (up to 15 s) before notifying
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(30);

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

enum Action {
    Status,
    Scan(usize),
    Write(Vec<u8>),
}

fn parse_args() -> Option<Action> {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let action = match args.as_slice() {
        ["status"] => Action::Status,
        ["scan"] => Action::Scan(1),
        ["scan", n] => Action::Scan(n.parse().ok()?),
        ["set", ssid] => Action::Write(set_wifi_json(ssid, "")),
        ["set", ssid, password] => Action::Write(set_wifi_json(ssid, password)),
        ["disconnect"] => Action::Write(serde_json::json!({ "cmd": "disconnect", "id": 1 }).to_string().into_bytes()),
        ["raw", json] => Action::Write(json.as_bytes().to_vec()),
        _ => return None,
    };
    Some(action)
}

fn set_wifi_json(ssid: &str, password: &str) -> Vec<u8> {
    serde_json::json!({ "cmd": "set_wifi", "id": 1, "ssid": ssid, "password": password })
        .to_string()
        .into_bytes()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::init();

    let Some(action) = parse_args() else {
        eprintln!("usage: gatt_client status | scan [n] | set <ssid> [password] | disconnect | raw <json>");
        std::process::exit(2);
    };

    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;

    let device = find_server(&adapter).await?;
    if !device.is_connected().await? {
        println!("Connecting to {} ...", device.address());
        device.connect().await?;
    }
    println!("Connected");

    let result = run(&device, action).await;

    // Always disconnect, success or not, so the server sees the connection end
    let _ = device.disconnect().await;
    result
}

async fn run(device: &Device, action: Action) -> Result<()> {
    match action {
        Action::Status => {
            let ch = find_characteristic(device, NETWORK_STATUS_UUID).await?;
            let value = ch.read().await?;
            println!("NetworkStatus: {}", String::from_utf8_lossy(&value));
        }
        Action::Write(payload) => {
            let ch = find_characteristic(device, COMMAND_UUID).await?;
            println!("Writing {} bytes: {}", payload.len(), String::from_utf8_lossy(&payload));
            // A ReqError returned by the server shows up here as Err, e.g. NotSupported
            match ch.write(&payload).await {
                Ok(()) => println!("Write OK"),
                Err(e) => println!("Write rejected by server: {e}"),
            }
        }
        Action::Scan(rounds) => {
            let ch = find_characteristic(device, AVAILABLE_NETWORKS_UUID).await?;
            receive_networks(&ch, rounds).await?;
        }
    }
    Ok(())
}

/// Subscribe to AvailableNetworks. The server splits each JSON payload into MTU-sized chunks
/// and ends the last one with '\n', so chunks are joined until a '\n' completes a round.
async fn receive_networks(ch: &Characteristic, rounds: usize) -> Result<()> {
    println!("Subscribing to AvailableNetworks (waiting for {rounds} round(s)) ...");
    let notify = ch.notify().await?;
    pin_mut!(notify);

    let mut buf = Vec::new();
    let mut done = 0;
    while done < rounds {
        let Some(chunk) = timeout(NOTIFY_TIMEOUT, notify.next()).await? else {
            println!("Notification session ended by server");
            break;
        };
        buf.extend_from_slice(&chunk);

        // A chunk should contain at most one '\n', but loop anyway to be safe
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            done += 1;
            print_networks(&line[..line.len() - 1], done);
        }
    }
    // bluer sends StopNotify automatically when the notify stream is dropped
    Ok(())
}

fn print_networks(json: &[u8], round: usize) {
    let networks: Vec<serde_json::Value> = match serde_json::from_slice(json) {
        Ok(v) => v,
        Err(e) => {
            println!("Round {round}: invalid JSON ({e}): {}", String::from_utf8_lossy(json));
            return;
        }
    };
    println!("Round {round}: {} networks", networks.len());
    for n in networks {
        println!("  {:<32} signal={:<4} secured={}", n["ssid"].as_str().unwrap_or("?"), n["signal"], n["secured"]);
    }
}

/// Use BLE_ADDR if set; otherwise scan for the first device advertising our service UUID
async fn find_server(adapter: &Adapter) -> Result<Device> {
    if let Ok(addr) = env::var("BLE_ADDR") {
        let addr: Address = addr.parse()?;
        println!("Using BLE_ADDR={addr}");
        // BlueZ only has a D-Bus object for the device after it has been discovered
        let events = adapter.discover_devices().await?;
        pin_mut!(events);
        timeout(DISCOVER_TIMEOUT, async {
            while let Some(event) = events.next().await {
                if matches!(event, AdapterEvent::DeviceAdded(a) if a == addr) {
                    break;
                }
            }
        })
        .await?;
        return Ok(adapter.device(addr)?);
    }

    println!("Discovering devices advertising {PRIMARY_SERVICE_UUID} ...");
    let events = adapter.discover_devices().await?;
    pin_mut!(events);

    let found = timeout(DISCOVER_TIMEOUT, async {
        while let Some(event) = events.next().await {
            let AdapterEvent::DeviceAdded(addr) = event else { continue };
            let Ok(device) = adapter.device(addr) else { continue };
            let uuids = device.uuids().await.ok().flatten().unwrap_or_default();
            if uuids.contains(&PRIMARY_SERVICE_UUID) {
                println!("Found server {addr} (name={:?})", device.name().await.ok().flatten());
                return Some(device);
            }
        }
        None
    })
    .await?;

    found.ok_or_else(|| "discovery stream ended without finding the server".into())
    // `events` is dropped here, which also stops discovery
}

async fn find_characteristic(device: &Device, uuid: Uuid) -> Result<Characteristic> {
    // services() waits until BlueZ has resolved the GATT database
    for service in device.services().await? {
        if service.uuid().await? != PRIMARY_SERVICE_UUID {
            continue;
        }
        for ch in service.characteristics().await? {
            if ch.uuid().await? == uuid {
                return Ok(ch);
            }
        }
    }
    Err(format!("characteristic {uuid} not found").into())
}
