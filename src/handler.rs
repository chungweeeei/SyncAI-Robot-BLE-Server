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
use std::time::Duration;
use tokio::time::sleep;
use nmrs::{NetworkManager, raw::zbus};
use serde::Serialize;

use crate::helper::parse_rssi_dbm;
use crate::scan;
use crate::setting::Command;

#[derive(Debug, Serialize)]
struct NetworkStatus {
    connected: bool,
    ssid: Option<String>,
    signal: Option<i32>, // RSSI in dBm, e.g. -50
    ip: Option<String>,
}

/// 從 /proc/net/wireless 讀取介面目前的 RSSI（dBm）
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

/// 每一輪送完之後，等多久再重新掃描
const RESCAN_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Serialize)]
struct AvailableNetwork {
    ssid: String,
    signal: Option<u8>, // NM strength, 0-100 (%)
    secured: bool,
}

/// AvailableNetworks characteristic 的 control loop（IO 模式）。
///
/// IO 模式沒有 callback，改成從 `CharacteristicControl` 這個 stream 收事件：
/// 每當有 client 訂閱，就會收到一個 `Notify(writer)`，
/// 每個訂閱各自開一個背景 task 去掃描和送資料。
pub async fn serve_available_networks(nm: NetworkManager, control: CharacteristicControl) {
    pin_mut!(control);
    while let Some(event) = control.next().await {
        match event {
            CharacteristicControlEvent::Notify(writer) => {
                tokio::spawn(notify_available_networks(nm.clone(), writer));
            }
            // 這個 characteristic 沒有開 write，理論上不會收到
            CharacteristicControlEvent::Write(req) => req.reject(ReqError::NotSupported),
        }
    }
}

async fn notify_available_networks(nm: NetworkManager, writer: CharacteristicWriter) {
    // IO 模式拿得到這個 client 協商後的 MTU，
    // bluer 的 send() 規定一次最多 mtu() bytes，所以直接用它當 chunk 大小
    let chunk_len = writer.mtu();
    println!("Notification session start from {} (mtu={chunk_len})", writer.device_address());

    let conn = match zbus::Connection::system().await {
        Ok(conn) => conn,
        Err(e) => {
            println!("Failed to connect to system D-Bus: {e}");
            return;
        }
    };

    // is_closed() 出錯也當作連線已經結束
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

/// Command characteristic 的 write callback。
///
/// 只負責解析和檢查 Command，格式錯誤就回傳錯誤給手機；
/// 真正耗時的操作（連線最多 30 秒）丟到背景 task，這裡馬上回傳 Ok，
/// 避免超過 ATT 的 30 秒逾時。
pub async fn write_command(
    _nm: NetworkManager,
    value: Vec<u8>,
    req: CharacteristicWriteRequest,
) -> ReqResult<()> {
    println!("Command write from {} ({} bytes, mtu={}, offset={})",
             req.device_address, value.len(), req.mtu, req.offset);

    // 還不支援分段寫入（Prepare Write），整個 Command 要一次寫完
    if req.offset != 0 {
        return Err(ReqError::InvalidOffset);
    }

    let cmd = Command::parse(&value).map_err(|e| {
        println!("Invalid command: {e:?}");
        ReqError::from(e)
    })?;

    match cmd {
        // 注意：不要直接印出 cmd，Debug 會把密碼也印出來
        // 先不真的切換網路，只確認 Command 有正確收到
        Command::SetWifi { id, ssid, .. } => {
            println!("[{id}] SetWifi received: ssid=\"{ssid}\"");
            Ok(())
        }
        Command::Disconnect { id } => {
            println!("[{id}] Disconnect: not implemented yet");
            Err(ReqError::NotSupported)
        }
    }
}
