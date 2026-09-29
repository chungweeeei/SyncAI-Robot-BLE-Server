# gatt_server_cb.rs 與 gatt_server_io.rs 的差異

範例位置：`~/.cargo/registry/src/index.crates.io-*/bluer-0.17.4/examples/`（下方的行號都對應 bluer 0.17.4）

**兩支範例的功能一樣**：一個 4 bytes 的值，手機寫入就會更新；訂閱 notify 之後，server 會定時把每個 byte 減 1 再推送給手機。差別在於**程式怎麼接收這些事件**。

## 一句話說明

- **cb.rs（callback 模式）**：你先寫好處理函式交給 bluer，**有請求進來時 bluer 會呼叫你的函式**。
- **io.rs（IO 模式）**：bluer 給你一條類似 socket 的**資料串流**，**你自己寫迴圈去讀、去寫**。

## 詳細比較

| | `gatt_server_cb.rs` | `gatt_server_io.rs` |
|---|---|---|
| 設定方式 | `CharacteristicWriteMethod::Fun(...)`<br>`CharacteristicNotifyMethod::Fun(...)` | `CharacteristicWriteMethod::Io`<br>`CharacteristicNotifyMethod::Io` |
| 支援 Read 嗎 | ✅ 有 `read` | ❌ 範例沒有 read。`CharacteristicRead` 只有 `fun` 欄位，**read 一定要用 callback** |
| 收到寫入 | closure 直接拿到整筆資料 `new_value: Vec<u8>` | 從 `CharacteristicReader` 用 `reader.read(&mut buf)` 讀，每次最多 MTU 大小 |
| 送出 notify | 手機訂閱時呼叫你的 closure，拿到 `notifier` 後自己 `tokio::spawn` 一個迴圈，用 `notifier.notify(vec)` 送出 | 在 `CharacteristicControlEvent::Notify` 事件拿到 `writer`，用 `writer.write(&bytes)` 送出 |
| 共用資料 | 多個 closure 共用同一個值，要用 `Arc<Mutex<Vec<u8>>>`（第 40 行） | 全部在同一個迴圈處理，直接用 `let mut value`，不用上鎖（第 79 行） |
| 額外設定 | 不需要 | 要用 `characteristic_control()` 建立控制器，填到 `control_handle`（第 44、62 行） |
| 主程式結構 | 註冊完就等使用者按 Enter，事件都在 callback 裡處理 | 一個大的 `tokio::select!` 迴圈，同時等 stdin、新事件、計時器、讀到資料（第 86–139 行） |
| 效能 | 每次讀寫都經過一次 D-Bus 訊息 | 官方：*"This has low overhead"*，資料直接走 socket |
| 多個手機訂閱 | 每次訂閱都 spawn 獨立的 task，互不影響 | 範例只有一個 `writer_opt`，新連線會蓋掉舊的 |

## 程式碼對照：處理寫入

**cb.rs**（第 66–75 行）：bluer 把資料送到你手上

```rust
method: CharacteristicWriteMethod::Fun(Box::new(move |new_value, req| {
    let value = value_write.clone();
    async move {
        let mut value = value.lock().await;   // 上鎖
        *value = new_value;                   // 直接拿到整筆資料
        Ok(())
    }.boxed()
})),
```

**io.rs**（第 91–94 行、第 117–137 行）：先接受連線，之後自己去讀

```rust
Some(CharacteristicControlEvent::Write(req)) => {
    read_buf = vec![0; req.mtu()];
    reader_opt = Some(req.accept()?);         // 拿到一條讀取串流
},
...
read_res = async {
    match &mut reader_opt {
        Some(reader) => reader.read(&mut read_buf).await,
        None => future::pending().await,      // 還沒有 reader 就一直等，不會觸發
    }
} => { ... }
```

`future::pending()` 是 `select!` 常見的技巧：還沒有 reader 時，這個分支永遠不會完成，所以不會被選到。

## 範例的初始值

```rust
let value = Arc::new(Mutex::new(vec![0x10, 0x01, 0x01, 0x10]));
```

`0x10, 0x01, 0x01, 0x10` **沒有特別意義**，只是範例挑的 4 個 bytes，當作 characteristic 的初始值，目的是**讓你看得出值有在變化**。notify 迴圈每 5 秒把每個 byte 減 1（第 97–99 行）：

```rust
for v in &mut *value {
    *v = v.saturating_sub(1);
}
```

```
[10, 01, 01, 10]
[0f, 00, 00, 0f]
[0e, 00, 00, 0e]   ← 0x00 減 1 還是 0x00
...
[00, 00, 00, 00]
```

`saturating_sub` 是「減到 0 就停住」。`u8` 沒有負數，直接寫 `v - 1` 的話，0 減 1 在 debug 模式會 panic，在 release 模式會繞回 255。

## 各自會學到的 Rust 觀念

- **cb.rs**：closure、`move`、`Box<dyn Fn>`、`Arc<Mutex<T>>`、`tokio::spawn`
- **io.rs**：`tokio::select!`、`Stream` 和 `StreamExt::next()`、`pin_mut!`、`Option` 狀態機、`AsyncRead` / `AsyncWrite`

## 該選哪個

- **指令和狀態回報**（資料量小）：**cb 模式**。程式直覺，也支援 read。
- **高頻率的串流資料**（感測器資料、影像片段）：IO 模式，吞吐量較好。

兩種可以混用，因為 `read`、`write`、`notify` 是分開設定的。例如 read 用 callback，write 和 notify 用 IO。
