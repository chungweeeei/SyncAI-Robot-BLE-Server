# bluer 基本用法

## 整體架構

```
Session（連到系統的 bluetoothd）
  └─ Adapter（藍牙晶片，通常是 hci0）
       ├─ advertise(Advertisement)            → 廣播：讓手機掃描得到
       └─ serve_gatt_application(Application) → GATT Server：讓手機連上來讀寫
            └─ Service
                 └─ Characteristic（read / write / notify）
```

---

## 第 1 層：Session 和 Adapter

```rust
#[tokio::main]
async fn main() -> bluer::Result<()> {
    let session = bluer::Session::new().await?;        // 連到 bluetoothd
    let adapter = session.default_adapter().await?;    // 取得預設的藍牙卡（hci0）
    adapter.set_powered(true).await?;                  // 打開電源

    println!("Adapter: {}", adapter.name());
    println!("Address: {}", adapter.address().await?);
    Ok(())
}
```

**Rust 語法**

| 語法 | 意思 |
|---|---|
| `#[tokio::main]` | 讓 `main` 可以寫成 `async`。bluer 的 API 全部都是非同步的，要靠 tokio 執行 |
| `async fn` / `.await` | 呼叫非同步函式，等它的結果回來 |
| `bluer::Result<()>` | 回傳值不是成功（`Ok(())`）就是失敗（`Err(錯誤)`） |
| `?` | 如果是 `Err` 就直接從函式 return 錯誤；如果是 `Ok` 就把裡面的值取出來 |

---

## 第 2 層：廣播（Advertisement）

廣播就像機器人在喊「我在這裡」，手機掃描時才看得到它。

```rust
use bluer::adv::Advertisement;

let adv = Advertisement {
    service_uuids: vec![SERVICE_UUID].into_iter().collect(),
    discoverable: Some(true),
    local_name: Some("SyncAI-Robot".to_string()),
    ..Default::default()        // 其他欄位都用預設值
};
let _adv_handle = adapter.advertise(adv).await?;
```

### 重點：handle 被丟掉（drop），廣播就會停止

官方文件：*"The advertisement remains active as long as the returned handle exists"*。

- `_adv_handle` 一定要一直存在，例如留在 `main` 裡。
- 如果寫成 `let _ = ...`，handle 會馬上被丟掉，廣播也會馬上停止。
- 這是 Rust 很重要的觀念 **RAII**：資源的生命週期跟變數綁在一起，變數離開作用範圍，資源就自動清掉。

**Rust 語法**

- `Option<T>`：`Some(值)` 代表有值，`None` 代表沒有值。Rust 沒有 null。
- `..Default::default()`：*struct update syntax*，沒寫出來的欄位都用預設值。bluer 的設定 struct 大量用到這個寫法。

---

## 第 3 層：GATT Application

一層一層描述「這個 server 有哪些 Service，每個 Service 有哪些 Characteristic」：

```rust
use bluer::gatt::local::{Application, Service, Characteristic, CharacteristicWrite, CharacteristicNotify};

let app = Application {
    services: vec![Service {
        uuid: SERVICE_UUID,
        primary: true,
        characteristics: vec![Characteristic {
            uuid: CHAR_UUID,
            write:  Some(CharacteristicWrite  { write: true,  /* method */ ..Default::default() }),
            notify: Some(CharacteristicNotify { notify: true, /* method */ ..Default::default() }),
            ..Default::default()
        }],
        ..Default::default()
    }],
    ..Default::default()
};
let _app_handle = adapter.serve_gatt_application(app).await?;   // 跟廣播一樣，handle 不能被丟掉
```

每個 Characteristic 的 `read`、`write`、`notify` 都是 `Option`：填 `Some(...)` 就開放，填 `None` 就不開放。

---

## 第 4 層：處理手機送來的請求

官方 README：*"a callback-based interface and low-overhead AsyncRead and AsyncWrite streams"*。

- **Callback 模式**（`gatt_server_cb.rs`）：你提供函式，手機來讀寫時 bluer 會呼叫它。建議先學這個。
- **IO 模式**（`gatt_server_io.rs`）：把 characteristic 當成串流，自己用 `read()` / `write()` 讀寫，效能較好。

Callback 模式的寫入範例：

```rust
write: Some(CharacteristicWrite {
    write: true,
    method: CharacteristicWriteMethod::Fun(Box::new(move |new_value, _req| {
        async move {
            println!("手機寫入: {:?}", new_value);
            Ok(())
        }.boxed()
    })),
    ..Default::default()
}),
```

**Rust 語法**

- `move |參數| { ... }`：**closure**（匿名函式），`move` 表示把用到的變數的所有權搬進 closure。
- `Box::new(...)`：把 closure 放到 heap 上。每個 closure 的型別都不一樣，要包起來才能存進同一個欄位。
- `.boxed()`：把 async 區塊包成統一的 Future 型別，來自 `futures::FutureExt`。
- 多個 callback 共用同一份資料時，要用 `Arc<Mutex<T>>`，請看 [06-rust-arc-mutex.md](06-rust-arc-mutex.md)。
