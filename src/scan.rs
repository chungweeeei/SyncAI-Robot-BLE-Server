use std::time::Duration;

use nmrs::{Network, NetworkManager, raw::zbus};
use tokio::time::{Instant, sleep};

const NM_SERVICE: &str = "org.freedesktop.NetworkManager";
const NM_WIRELESS_IFACE: &str = "org.freedesktop.NetworkManager.Device.Wireless";

/// Max time to wait for a scan to finish; after that, use whatever list NM currently has
const SCAN_TIMEOUT: Duration = Duration::from_secs(15);
const SCAN_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Trigger a Wi-Fi rescan and return the visible networks once the scan completes.
///
/// `nm.scan_networks()` returns right after sending RequestScan without waiting for the scan,
/// so this reads NM's `LastScan` property (updated whenever a scan finishes) and fetches the
/// list once it changes.
///
/// `conn` is our own system bus connection (nmrs doesn't expose its internal one); the caller
/// creates it once and reuses it.
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
        // NM rejects RequestScan calls that come too often; just reuse NM's current results
        Err(e) => println!("Wi-Fi rescan request failed, using cached results: {e}"),
    }

    let mut networks = wifi.list_networks().await?;
    networks.retain(|n| !n.ssid.is_empty()); // hidden SSIDs
    networks.sort_by(|a, b| b.strength.cmp(&a.strength));
    Ok(networks)
}
