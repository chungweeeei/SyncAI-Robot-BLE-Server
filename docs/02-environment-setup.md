# 開發環境設定

## 注意：bluer 只能在 Linux 上編譯

`bluer` 透過 **D-Bus** 跟 Linux 的 BlueZ（`bluetoothd`）溝通，所以在 macOS 上一定會編譯失敗。要在 Ubuntu（實體機、VM 或 Docker）上 build。

## Ubuntu 安裝步驟

```bash
# 1. 安裝 BlueZ 和編譯需要的系統套件
sudo apt update
sudo apt install bluez libdbus-1-dev pkg-config build-essential

# 2. 確認藍牙服務有在跑
sudo systemctl enable --now bluetooth
bluetoothctl show      # 看得到 adapter 就代表沒問題

# 3. 編譯
cargo build
```

- GATT server 和廣播功能需要 BlueZ **5.50 以上**，可以用 `bluetoothd --version` 查。
- 執行時如果遇到權限錯誤，可以先用 `sudo` 跑，或者把使用者加進 `bluetooth` 群組。

## 官方建議的 BlueZ 設定

bluer 的 README 建議在 `/etc/bluetooth/main.conf` 加上：

```ini
[GATT]
Cache = no      # 關閉 GATT 快取，避免手機讀到舊的 service 資料
Channels = 1    # 關閉 EATT，讓資料照順序傳送，比較好除錯
```

改完之後重啟藍牙服務：

```bash
sudo systemctl restart bluetooth
```

## 在 UTM（VM）上測試

可以，但 **VM 用不到 Mac 內建的藍牙**，要另外準備一支 **USB 藍牙 dongle**，再直通（passthrough）給 VM。

UTM 只能直通 **USB 裝置**，Mac 內建的藍牙晶片不是 USB 裝置，所以 VM 裡會看不到任何 adapter。

| 項目 | 只用 UTM | UTM + USB dongle |
|---|---|---|
| `cargo build` 編譯 | ✅ | ✅ |
| 開 GATT server、廣播 | ❌ 沒有 adapter | ✅ |
| 用手機連上來測試 | ❌ | ✅ |

### 設定步驟

1. **VM 要用 QEMU 模式**：建立 VM 時選 **Emulate/Virtualize → QEMU**，不要選 Apple Virtualization（不支援 USB 直通）。
2. VM 開機後插上 dongle，在 UTM 工具列的 **USB 圖示**點選那支 dongle，連到 VM。
3. 在 Ubuntu 裡確認：
   ```bash
   lsusb              # 看得到藍牙 dongle
   bluetoothctl show  # 看得到 adapter
   ```

### Dongle 怎麼挑

- **Realtek RTL8761B/BU**（BT 5.0）：便宜、常見，Linux 支援好。
- **CSR8510**（BT 4.0）：BLE server 夠用，幾乎不用額外裝驅動。
- 插上後 `bluetoothctl` 還是看不到 adapter，通常是少了韌體：`sudo apt install linux-firmware`。

USB 直通有時不太穩，dongle 載入韌體時會重新連接，UTM 可能要再手動連一次。如果手邊有樹莓派之類的實體 Linux 機器，直接在上面測會比較省事。

## 測試工具

手機安裝 **nRF Connect**（iOS / Android 都有），可以掃描、連線、讀寫 characteristic、訂閱 notify。
