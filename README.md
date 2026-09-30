# SyncAI-Robot-BLE-Server

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
