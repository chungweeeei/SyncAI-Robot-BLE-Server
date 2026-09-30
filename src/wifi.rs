use nmrs::{NetworkManager, WifiSecurity};

/// 切換到指定的 Wi-Fi。密碼是空字串就當作開放網路，否則用 WPA-PSK。
///
/// 會等到 NM 連線成功或失敗才回傳（nmrs 預設最多等 30 秒），
/// 所以不能直接在 BLE 的 write callback 裡 await，要丟到背景 task 執行。
pub async fn switch_network(nm: &NetworkManager, ssid: &str, password: &str) -> nmrs::Result<()> {
    let security = if password.is_empty() {
        WifiSecurity::Open
    } else {
        WifiSecurity::WpaPsk { psk: password.to_string() }
    };
    nm.connect(ssid, None, security).await
}
