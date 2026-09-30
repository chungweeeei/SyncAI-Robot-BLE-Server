# CLAUDE.md

機器人上的 BLE GATT server（Rust），讓手機透過藍牙查詢/設定 Wi-Fi。藍牙用 `bluer`（BlueZ），網路用 `nmrs`（NetworkManager）。目標平台是 Ubuntu 22.04。GATT 介面與 Command 格式見 `README.md`。

## 常用指令

主機不裝 Rust，一律透過容器跑 cargo（第一次會自動 build image）：

```bash
scripts/cargo build
scripts/cargo clippy
scripts/cargo test
scripts/cargo fmt
scripts/cargo run        # 透過主機 D-Bus 使用主機的 BlueZ / NM
```

- 修改 `docker/cargo/Dockerfile` 後：`CARGO_IMAGE_REBUILD=1 scripts/cargo build`
- 開發機若是 macOS，無法實際執行 BLE / NM 相關功能，只能 build、clippy 和跑單元測試。
- 實機測試要用另一台 Linux 跑 `cargo run --example gatt_client -- <status|scan|set|disconnect|raw>`。
- NM 操作出現 `not authorized` 時，是 polkit 權限問題，見 README 的「NetworkManager 權限」。

## 架構

- `main.rs`：建立 NM 與 bluer session、廣播、組 `Application`，stdin 按 enter 結束（drop handle 會移除 service/廣播）。
- `handler.rs`：
  - `read_network_status`：cb 模式 read，回傳 JSON。
  - `serve_available_networks`：IO 模式 notify，每個訂閱 spawn 一個 task，rescan → JSON → 依 `writer.mtu()` 切段送出，以 `\n` 結尾。
  - `write_command`：cb 模式 write，只負責解析/驗證；耗時操作（NM 連線最多 30 秒）必須丟到背景 task，否則會超過 ATT 30 秒逾時。
- `setting.rs`：`Command`（serde `tag = "cmd"`、`snake_case`、`deny_unknown_fields`）與 `CommandError → ReqError` 的對應。
- `scan.rs`：`nm.scan_networks()` 不會等掃描完成，所以另外讀 D-Bus 的 `LastScan` 屬性輪詢，最多等 15 秒。
- `wifi.rs`：`switch_network`，空密碼視為開放網路，否則 WPA-PSK。
- `config.rs`：UUID。**`examples/gatt_client.rs` 有一份複製的 UUID**（binary crate 沒有 lib.rs），改 UUID 時兩邊要同步。

目前進度：`set_wifi` 只驗證、尚未呼叫 `wifi::switch_network`；`disconnect` 回 `NotSupported`。

## 慣例

- Runtime 是 `tokio` 的 `current_thread`。
- Lint：`unsafe_code = "forbid"`、`unused_must_use = "deny"`、clippy `all` + `pedantic`（warn）。改完要跑 `scripts/cargo clippy` 確認沒有新 warning。
- 格式：`rustfmt.toml`（`max_width = 100`）。
- 註解與文件用繁體中文，風格簡潔。
- Commit message 遵循 Conventional Commits（`feat:`、`fix:`、`docs:`、`build:`、`chore:` …，可加 scope），見 `.github/prompt/copilot-commit-message-instructions.md`。
- 分支：功能開在 feature branch，PR 回 `dev`。
- `docs/` 是學習筆記（bluer、Rust 觀念、設計），不是 API 文件。
