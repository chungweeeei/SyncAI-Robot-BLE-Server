//! 測試用的 GATT client，在「另一台」有藍牙的 Linux 上跑（同一張 adapter 不能連自己）。
//!
//! 用法：
//!   cargo run --example gatt_client -- status
//!   cargo run --example gatt_client -- scan [輪數，預設 1]
//!   cargo run --example gatt_client -- set <ssid> [password]
//!   cargo run --example gatt_client -- disconnect
//!   cargo run --example gatt_client -- raw '<json>'     # 送任意內容，測試錯誤處理
//!
//! 預設會掃描廣播 PRIMARY_SERVICE_UUID 的裝置；
//! 設定 BLE_ADDR=AA:BB:CC:DD:EE:FF 可以直接指定要連的 server。

use std::{env, time::Duration};

use bluer::{Adapter, AdapterEvent, Address, Device, Uuid, gatt::remote::Characteristic};
use futures::{StreamExt, pin_mut};
use tokio::time::timeout;

// examples 沒辦法 `use` binary crate 裡的 module（沒有 lib.rs），
// 所以 UUID 先複製一份，要跟 src/config.rs 保持一致。
const PRIMARY_SERVICE_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_56789abcdef0);
const COMMAND_UUID: Uuid = Uuid::from_u128(0x1234abcd_0000_0000_8000_00805f9b34fb);
const NETWORK_STATUS_UUID: Uuid = Uuid::from_u128(0x1234abcd_0001_0000_8000_00805f9b34fb);
const AVAILABLE_NETWORKS_UUID: Uuid = Uuid::from_u128(0x1234abcd_0002_0000_8000_00805f9b34fb);

const DISCOVER_TIMEOUT: Duration = Duration::from_secs(20);
/// server 每一輪要先等 Wi-Fi 掃描完（最多 15 秒）才會 notify
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

    // 不管成功或失敗都斷線，server 端才看得到連線結束
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
            // server 回傳的 ReqError 會變成這裡的 Err，例如 NotSupported
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

/// 訂閱 AvailableNetworks。server 會把一份 JSON 切成 20 bytes 一段送過來，
/// 最後一段以 '\n' 結尾，所以這裡把收到的片段接起來，遇到 '\n' 才算一輪。
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

        // 一個 chunk 裡理論上只會有一個 '\n'，但用迴圈處理比較保險
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            done += 1;
            print_networks(&line[..line.len() - 1], done);
        }
    }
    // notify 這個 stream 被 drop 時，bluer 會自動送出 StopNotify
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

/// 有設定 BLE_ADDR 就直接用；否則開始掃描，找第一個廣播我們 service UUID 的裝置
async fn find_server(adapter: &Adapter) -> Result<Device> {
    if let Ok(addr) = env::var("BLE_ADDR") {
        let addr: Address = addr.parse()?;
        println!("Using BLE_ADDR={addr}");
        // 要先掃描過，BlueZ 才會有這個裝置的 D-Bus 物件
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
    // `events` 在這裡被 drop，掃描也跟著停止
}

async fn find_characteristic(device: &Device, uuid: Uuid) -> Result<Characteristic> {
    // services() 會等 BlueZ 把 GATT database 解析完才回傳
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
