# SyncAI-Robot-BLE-Server

在機器人（Ubuntu 22.04）上執行的 BLE GATT server，讓手機透過藍牙查詢和設定機器人的 Wi-Fi。

- 藍牙：透過 [bluer](https://docs.rs/bluer) 操作主機的 BlueZ
- 網路：透過 [nmrs](https://docs.rs/nmrs) 操作 NetworkManager

## GATT 介面

Primary service UUID：`12345678-1234-5678-1234-56789abcdef0`（廣播名稱目前是 `test`）

| Characteristic | UUID | 屬性 | 內容 |
|---|---|---|---|
| NetworkStatus | `1234abcd-0001-0000-8000-00805f9b34fb` | read | `{"connected":true,"ssid":"...","signal":-50,"ip":"192.168.1.23"}`，`signal` 是 RSSI（dBm） |
| AvailableNetworks | `1234abcd-0002-0000-8000-00805f9b34fb` | notify（IO 模式） | 訂閱後每輪 rescan 推送一次 `[{"ssid":"...","signal":80,"secured":true}, ...]`，`signal` 是 NM 強度（0-100），依 MTU 切段，以 `\n` 結尾 |
| Command | `1234abcd-0000-0000-8000-00805f9b34fb` | write | JSON 指令，最長 512 bytes，必須一次寫完（offset 要是 0） |

Command 格式：

```json
{"cmd": "set_wifi", "id": 1, "ssid": "MyWiFi", "password": "secret"}
{"cmd": "disconnect", "id": 2}
```

- `ssid` 長度 1-32 bytes、`password` 最多 63 bytes，空字串代表開放網路。
- 格式錯誤回 `NotSupported`、太長回 `InvalidValueLength`、SSID/密碼不合法回 `Failed`。
- 目前進度：`set_wifi` 只解析和驗證（還沒真的切換網路），`disconnect` 尚未實作。

## 專案結構

```
src/
  main.rs      建立 NM / BlueZ session、廣播、註冊 GATT application
  config.rs    Service 與 characteristic 的 UUID
  handler.rs   三個 characteristic 的 read / notify / write 處理
  setting.rs   Command 的 JSON 解析與驗證
  scan.rs      觸發 Wi-Fi rescan 並等待 NM 的 LastScan 更新
  wifi.rs      透過 NM 切換 Wi-Fi
  helper.rs    解析 /proc/net/wireless 取得 RSSI
examples/
  gatt_client.rs   測試用 GATT client
deploy/polkit/     NetworkManager 的 polkit 規則
docker/cargo/      cargo 開發容器
docs/              學習筆記（見 docs/README.md）
```

## Build（不用在主機安裝 Rust）

用 `scripts/cargo` 在容器裡執行 cargo，用法跟 cargo 一樣：

```bash
scripts/cargo build
scripts/cargo clippy
scripts/cargo test
```

執行 BLE server 有兩種方式，都是透過主機的 BlueZ（`bluetooth.service`）：

```bash
scripts/cargo run                                  # 在容器裡跑（已掛入主機的 D-Bus socket）
scripts/cargo build && ./target/debug/SyncAI-Robot-BLE-Server   # 在主機上直接跑
```

- 第一次執行會自動 build image（`docker/cargo/Dockerfile`）。修改 Dockerfile 後用 `CARGO_IMAGE_REBUILD=1 scripts/cargo build` 重建。
- image 的 base 是 `ubuntu:22.04`，和主機的 glibc 一樣，所以編出來的 `target/debug/SyncAI-Robot-BLE-Server` 可以直接在主機上執行（BLE 要用到主機的 BlueZ）。
- crate 下載快取放在 docker volume `syncai-ble-cargo-registry` / `syncai-ble-cargo-git`。

## NetworkManager 權限（polkit）

BLE server 透過 NetworkManager 掃描、連線和儲存 Wi-Fi 設定。NM 每個操作都會詢問 polkit，而預設只允許**本機登入**的使用者。從 SSH、容器或 systemd 啟動的程式都不算本機登入，會得到：

```
org.freedesktop.NetworkManager.wifi.scan request failed: not authorized
```

在主機上或容器裡執行都一樣會遇到這個錯誤，要用下面兩種方式之一解決。

### 方式 1：安裝 polkit 規則（開發時建議）

允許 `syncrobotic` 這個帳號不用密碼就能執行以下三個操作：

| polkit action | 用途 |
|---|---|
| `org.freedesktop.NetworkManager.wifi.scan` | rescan（AvailableNetworks） |
| `org.freedesktop.NetworkManager.network-control` | 連線、斷線（SetWifi、Disconnect） |
| `org.freedesktop.NetworkManager.settings.modify.system` | 儲存新的 Wi-Fi 設定檔（SetWifi） |

```bash
sudo install -m 644 deploy/polkit/50-syncai-ble-server.pkla /etc/polkit-1/localauthority/50-local.d/
sudo systemctl restart polkit
```

確認有沒有生效，三個都要顯示 `yes`：

```bash
nmcli general permissions | grep -E 'wifi.scan|network-control|settings.modify.system'
```

注意：

- Ubuntu 22.04 的 polkit 是 0.105，只支援 `.pkla` 格式，**不能用** `/etc/polkit-1/rules.d/*.rules`（新版 polkit 的 JavaScript 格式）。
- 規則裡寫的是 `Identity=unix-user:syncrobotic`。如果用別的帳號執行 server，要修改 `deploy/polkit/50-syncai-ble-server.pkla` 再重新安裝。
- 這個帳號的**所有程式**都會取得這些權限，不只是 BLE server。
- 如果顯示的還是 `auth` 或 `no`，先用 `sudo ls -l /etc/polkit-1/localauthority/50-local.d/` 確認檔案有沒有放對位置，再確認 polkit 有重啟。

### 方式 2：用 root 執行

NM 對 root 一律放行，不需要 polkit 規則：

```bash
sudo ./target/debug/SyncAI-Robot-BLE-Server
```

正式部署時，通常會用 systemd service 以 root 或專用帳號身分執行。

## 用 GATT client 測試

`examples/gatt_client.rs` 要在**另一台**有藍牙的 Linux 上跑（同一張 adapter 不能連自己）：

```bash
cargo run --example gatt_client -- status              # 讀 NetworkStatus
cargo run --example gatt_client -- scan 3              # 訂閱 AvailableNetworks，收 3 輪
cargo run --example gatt_client -- set <ssid> [password]
cargo run --example gatt_client -- disconnect
cargo run --example gatt_client -- raw '<json>'        # 送任意內容，測試錯誤處理
```

預設會掃描廣播 primary service UUID 的裝置，也可以用 `BLE_ADDR=AA:BB:CC:DD:EE:FF` 指定 server。

也可以用手機的 **nRF Connect** App 直接連線測試。
