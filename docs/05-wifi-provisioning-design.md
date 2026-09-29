# 網路配置（Wi-Fi Provisioning）設計

目標：手機透過 BLE 把 Wi-Fi 設定（SSID、密碼）傳給機器人，機器人連線後回報結果。

## 選擇 callback 模式

這種功能是「偶爾寫一次設定、回一個結果」，不是持續的資料流，所以用 **cb 模式**。

| 需求 | cb 模式 | IO 模式 |
|---|---|---|
| 手機寫入 SSID 和密碼（一次一筆） | ✅ closure 直接拿到整筆 `Vec<u8>` | 要自己讀串流、組封包 |
| 手機讀取目前的連線狀態 | ✅ 支援 `read` | ❌ 不支援 read |
| 回報連線成功或失敗 | ✅ 用 notify 推送 | 可以，但要自己寫 `select!` 迴圈 |
| 效能 | 一次只有幾百 bytes，D-Bus 的負擔可以忽略 | 高吞吐量用不到 |

## GATT 設計

```
Service: Network Provisioning (自訂 UUID)
 ├─ Config  (write)         手機寫入 {"ssid":"...","password":"..."}
 └─ Status  (read + notify)  "idle" / "connecting" / "connected:192.168.1.23" / "failed:..."
```

流程：

1. 手機連上機器人，訂閱 Status。
2. 手機寫入 Config。
3. 機器人檢查格式，開始連線，用 notify 回報 `connecting`。
4. 連線結束後用 notify 回報 `connected:<IP>` 或 `failed:<原因>`。

## 注意事項

### 1. callback 要馬上 return，不要在裡面等連線完成

連上 Wi-Fi 可能要好幾秒，BLE 的寫入請求有逾時限制（ATT 協定是 30 秒）。write callback 只做「解析設定、確認格式」，然後用 `tokio::spawn` 在背景執行連線（例如呼叫 Ubuntu 的 `nmcli`），結果再用 Status 的 notify 回報。

```rust
method: CharacteristicWriteMethod::Fun(Box::new(move |data, req| {
    async move {
        let cfg = parse(&data)?;                              // 格式錯誤就直接回傳錯誤給手機
        tokio::spawn(async move { apply_wifi(cfg).await });   // 在背景連線
        Ok(())                                                // 馬上回應手機「收到了」
    }.boxed()
})),
```

### 2. 資料長度和 MTU

- SSID 最多 32 bytes，密碼最多 63 bytes，包成 JSON 大約 100 多 bytes。
- BLE 預設 MTU 只有 23，扣掉標頭只能放 20 bytes。
- 現在的手機連線時通常會自動協商成較大的 MTU（iOS 常見 185，Android 可以要求到 517），但不能完全依賴。
- `CharacteristicWriteRequest` 有 `mtu` 和 `offset` 欄位，可以先把 `req.mtu` 印出來確認。
- 如果真的太小：在 App 端要求較大的 MTU，或把 SSID 和密碼拆成兩個 characteristic 分開寫入。

### 3. 密碼的安全性

BLE 預設是**明文傳輸**，旁邊有人用 sniffer 就能看到密碼。

- `CharacteristicWrite` 有 `encrypt_write: true` 選項，打開後手機必須先配對、在加密連線下才能寫入。**至少要打開這個選項。**
- 要更安全的話，可以在應用層再加一層加密。

## 用 channel 取代共享狀態

比起用 `Arc<Mutex>` 共享狀態，也可以用 `tokio::sync::mpsc` channel，把設定送給專門的背景 task：

```
write callback ──tx.send(WifiConfig)──▶ [channel] ──▶ 背景 task 負責執行 nmcli 連線
                                                       └─▶ 用 notify 回報結果
```

write callback 只負責把設定送進 channel 然後馬上 return，真正的工作交給背景 task。這樣自然做到了「callback 不要卡住」。
