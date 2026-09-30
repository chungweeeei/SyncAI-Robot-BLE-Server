use std::time::Duration;

use nmrs::{Network, NetworkManager, raw::zbus};
use tokio::time::{Instant, sleep};

const NM_SERVICE: &str = "org.freedesktop.NetworkManager";
const NM_WIRELESS_IFACE: &str = "org.freedesktop.NetworkManager.Device.Wireless";

/// 等待掃描完成的上限，超過就直接用目前 NM 手上的清單
const SCAN_TIMEOUT: Duration = Duration::from_secs(15);
const SCAN_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// 觸發 Wi-Fi rescan，等掃描完成後回傳目前看得到的網路。
///
/// `nm.scan_networks()` 只會送出 RequestScan 就回傳，不會等掃描結束，
/// 所以這裡另外讀 NM 的 `LastScan` 屬性（每次掃描完成都會更新），
/// 等它改變才去拿清單。
///
/// `conn` 是自己開的 system bus 連線（nmrs 沒有公開它內部的連線），
/// 由呼叫端建立一次後重複使用。
pub async fn rescan(nm: &NetworkManager, conn: &zbus::Connection) -> nmrs::Result<Vec<Network>> {
    let Some(dev) = nm.list_wireless_devices().await?.into_iter().next() else {
        return Ok(Vec::new());
    };
    let wifi = nm.wifi(&dev.interface);

    let proxy = zbus::Proxy::new(conn, NM_SERVICE, dev.path.as_str(), NM_WIRELESS_IFACE).await?;
    let last_scan = || async { proxy.get_property::<i64>("LastScan").await };

    let before = last_scan().await?;
    match wifi.scan().await {
        Ok(()) => {
            let deadline = Instant::now() + SCAN_TIMEOUT;
            while Instant::now() < deadline {
                sleep(SCAN_POLL_INTERVAL).await;
                if last_scan().await? != before {
                    break;
                }
            }
        }
        // NM 會拒絕太頻繁的 RequestScan，這時沿用 NM 目前的結果就好
        Err(e) => println!("Wi-Fi rescan request failed, using cached results: {e}"),
    }

    let mut networks = wifi.list_networks().await?;
    networks.retain(|n| !n.ssid.is_empty()); // 隱藏的 SSID
    networks.sort_by(|a, b| b.strength.cmp(&a.strength));
    Ok(networks)
}
