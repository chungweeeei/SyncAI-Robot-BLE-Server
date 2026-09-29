# 學習筆記

這個資料夾整理了開發 SyncAI Robot BLE Server 時的學習筆記，內容包含 BLE 函式庫的選擇、bluer 的用法，以及過程中碰到的 Rust 觀念。

## 目錄

| 文件 | 內容 |
|---|---|
| [01-rust-ble-libraries.md](01-rust-ble-libraries.md) | BLE 的 Central / Peripheral 角色、Rust BLE 函式庫比較、為什麼選 bluer |
| [02-environment-setup.md](02-environment-setup.md) | Ubuntu 環境安裝、BlueZ 設定、在 UTM VM 上測試 |
| [03-bluer-basics.md](03-bluer-basics.md) | bluer 的四層架構：Session → Adapter → Advertisement → GATT Application |
| [04-gatt-server-cb-vs-io.md](04-gatt-server-cb-vs-io.md) | 官方範例 `gatt_server_cb.rs` 與 `gatt_server_io.rs` 的差異 |
| [05-wifi-provisioning-design.md](05-wifi-provisioning-design.md) | 用 BLE 配置機器人網路設定的 GATT 設計 |
| [06-rust-arc-mutex.md](06-rust-arc-mutex.md) | `Arc`、`Mutex`、RAII，以及跟 C++ / Python / Go 的比較 |

## 學習路線

1. **Step 1**：只做 Session 和 Adapter，把 adapter 的資訊印出來，先熟悉 async 和 `?`。
2. **Step 2**：加上廣播，用手機的 **nRF Connect** App 掃描看看能不能看到機器人，學 `Option` 和 RAII。
3. **Step 3**：加一個可寫的 characteristic（callback 模式），在 nRF Connect 裡寫資料，看 server 有沒有印出來，學 closure。
4. **Step 4**：加上 notify，讓機器人定時回報狀態，學 `tokio::spawn` 和 `Arc<Mutex>`。
5. **Step 5**：實作網路配置功能，接上 `nmcli`。

## 參考資源

- bluer API 文件：https://docs.rs/bluer
- bluer 官方範例：https://github.com/bluez/bluer/tree/master/bluer/examples
- 本機的範例原始碼（`cargo` 下載後）：`~/.cargo/registry/src/index.crates.io-*/bluer-0.17.4/examples/`
