# Rust BLE 函式庫比較

## BLE 的兩種角色

| 角色 | 在做什麼 | 例子 |
|---|---|---|
| **Central / GATT Client** | 掃描、連線、讀寫別人的 characteristic | 手機 App 連手環 |
| **Peripheral / GATT Server** | 廣播、提供 service / characteristic、等別人來連 | 手環、心率帶、**這個專案的機器人** |

GATT 的階層：

```
Service（用 UUID 分辨）
 └─ Characteristic（實際的資料欄位，可設定 read / write / notify）
     └─ Descriptor
```

## 常見的函式庫

| 函式庫 | 角色 | 平台 | 說明 |
|---|---|---|---|
| `btleplug` | 只有 Central | macOS、Linux、Windows、iOS、Android | 跨平台、社群最多人用，但**不能當 GATT Server** |
| `bluer` | Central + **Peripheral** | **只支援 Linux** | BlueZ 官方的 Rust 綁定，GATT server、廣播、L2CAP 功能最完整 |
| `ble-peripheral-rust` | Peripheral | macOS、Linux、Windows | 跨平台的 peripheral 方案，但專案很新（0.2.0），文件較少 |
| `trouble` | Central + Peripheral | 嵌入式 / `no_std` | embassy 生態系，給 nRF52、ESP32、RP2040 等 MCU 使用 |
| `bluest` | Central | 跨平台 | 類似 btleplug，API 比較貼近各平台原生介面 |

## 怎麼選

- 跑在 **Linux** 上、要當 GATT server → **`bluer`**
- 想在 **Mac** 上直接開發測試 → `ble-peripheral-rust`
- BLE 在 **MCU** 上 → `trouble`
- 要去連**別的**裝置 → `btleplug`

## 本專案的決定

機器人跑在 **Ubuntu** 上，而且**只需要當 server**，不需要去連別的裝置，所以選擇 **`bluer`**。

```toml
[dependencies]
bluer = { version = "0.17.4", features = ["full"] }
futures = "0.3.34"
tokio = { version = "1.53.1", features = ["full"] }
```

- `bluer` 的 API 全部都是非同步的，要搭配 `tokio` 執行。
- `futures` 提供 `FutureExt::boxed()`、`StreamExt::next()` 等工具，官方範例都會用到。
