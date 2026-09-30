use nmrs::{NetworkManager, WifiSecurity};

/// Switch to the given Wi-Fi. An empty password means an open network, otherwise WPA-PSK.
///
/// Returns only after NM connects or fails (nmrs waits up to 30 s by default), so don't await
/// it directly in a BLE write callback; run it in a background task instead.
pub async fn switch_network(nm: &NetworkManager, ssid: &str, password: &str) -> nmrs::Result<()> {
    let security = if password.is_empty() {
        WifiSecurity::Open
    } else {
        WifiSecurity::WpaPsk { psk: password.to_string() }
    };
    nm.connect(ssid, None, security).await
}
