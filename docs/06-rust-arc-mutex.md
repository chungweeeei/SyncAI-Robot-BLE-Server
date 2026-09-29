# Rust 觀念：Arc、Mutex 與 RAII

以 `gatt_server_cb.rs` 的這幾行為例：

```rust
let value = Arc::new(Mutex::new(vec![0x10, 0x01, 0x01, 0x10]));
let value_read = value.clone();
let value_write = value.clone();
let value_notify = value.clone();
```

目的是解決 Rust 的一個核心問題：**同一份資料，要給好幾個 closure 一起使用**。

---

## Arc 是什麼

`Arc` **不是外部套件**，是 Rust 標準函式庫的一部分：

```rust
use std::sync::Arc;
```

**A**tomically **R**eference **C**ounted，「用原子操作做引用計數的智慧指標」。

- 讓**同一份資料同時有好幾個擁有者**。
- 內部記錄「現在有幾個擁有者」，最後一個被丟掉時資料才釋放。
- 計數的增減是執行緒安全的，可以跨執行緒使用。只在單一執行緒用的版本叫 `Rc`。

官方文件：https://doc.rust-lang.org/std/sync/struct.Arc.html

## 為什麼需要 Arc：所有權規則

Rust 規定**每個值同一時間只能有一個擁有者**。三個 closure 都寫了 `move`，會把變數整個搬進去：

```rust
let value = vec![0x10, 0x01, 0x01, 0x10];

let read_fn  = move || { /* 用 value */ };   // value 被搬進 read_fn
let write_fn = move || { /* 用 value */ };   // ❌ 編譯錯誤：value 已經被搬走了
```

解法是每個 closure 各給一個 `Arc`。**`Arc` 的 `.clone()` 不會複製資料**，只是多做一個指向同一份資料的指標，計數加 1：

```
value        ─┐
value_read   ─┼──▶ [ 計數: 4 | Mutex< [0x10, 0x01, 0x01, 0x10] > ]   ← 只有這一份
value_write  ─┤
value_notify ─┘
```

所以手機寫入之後，read 讀到的是新值，notify 推送的也是新值。

### closure 裡為什麼又 clone 一次？

```rust
fun: Box::new(move |req| {
    let value = value_read.clone();   // 又 clone 一次
    async move { ... }
```

這個 closure **每次手機來讀取時都會被呼叫**，裡面的 `async move` 會把 `value` 搬進回傳的 Future。如果直接搬走 `value_read`，第一次呼叫之後 closure 就沒得用了。

**第一次 clone 是給每個 closure 各一份，closure 裡的 clone 是給每次請求各一份。**

---

## Mutex 與 Mutex::new

### `new` 只是一般的函式名稱

Rust **沒有 `new` 關鍵字**。`Mutex::new` 是 `Mutex` 型別上一個叫 `new` 的函式，習慣上「建立新物件」的函式都叫 `new`：

```rust
Mutex::new(資料)   // 建立一個 Mutex，把資料包進去
Arc::new(資料)     // 建立一個 Arc，把資料包進去
Vec::new()         // 建立一個空的 Vec
```

`型別::函式()` 這種寫法叫 **associated function**，類似其他語言的靜態方法。

### 為什麼要 Mutex

`Arc` 只能讓大家**共同讀取**，不能修改，否則會 data race。`Mutex` 保證**同一時間只有一個地方能修改**。

| | 解決什麼問題 |
|---|---|
| `Arc<T>` | 讓**多個擁有者**共享同一份資料 |
| `Mutex<T>` | 讓共享的資料可以**安全地修改** |
| `Arc<Mutex<T>>` | 多個地方共享，而且都能修改 |

### Rust 的 Mutex 會包住資料

```rust
let m = Mutex::new(vec![0x10, 0x01]);

m.push(0x02);                   // ❌ 編譯錯誤：資料被包在 Mutex 裡面
m.lock().await.push(0x02);      // ✅ 先 lock() 拿到鎖，才能碰到裡面的 Vec
```

整行由內往外：

```
vec![...]             4 bytes 的資料
  └ Mutex::new(...)   包一層鎖 → 可以安全地修改
      └ Arc::new(...) 再包一層 → 可以給好幾個 closure 共享
```

### 為什麼用 tokio 的 Mutex

範例用的是 **`tokio::sync::Mutex`**，不是 `std::sync::Mutex`。因為 notify 迴圈在**拿著鎖的時候**呼叫了 `notifier.notify(...).await`。標準函式庫的鎖不適合跨 `.await` 一直拿著，tokio 的可以，所以 `lock()` 後面要加 `.await`。

---

## 跟 C++ / Python / Go 的比較

**C++**
```cpp
std::mutex mtx;
std::vector<uint8_t> value;     // 資料和鎖是分開的

mtx.lock();
value.push_back(0x02);
mtx.unlock();                   // 忘記寫，或中間 throw 例外 → deadlock

// 或用 RAII 的寫法
{
    std::lock_guard<std::mutex> guard(mtx);   // 建構時上鎖
    value.push_back(0x02);
}                                             // 解構時自動解鎖
```

**Python**
```python
lock.acquire()
value.append(0x02)
lock.release()          # 一樣有忘記解鎖的風險

with lock:              # 進入時上鎖
    value.append(0x02)
                        # 離開區塊時自動解鎖，即使發生例外也一樣
```

**Go**
```go
type State struct {
    mu    sync.Mutex
    value []byte          // 資料和鎖分開，習慣上放在同一個 struct
}

func (s *State) Write(b []byte) {
    s.mu.Lock()
    defer s.mu.Unlock()   // 函式 return 時才解鎖
    s.value = b
}
```

**Rust**
```rust
let mut guard = value.lock().await;   // 上鎖，拿到一個 guard
guard.push(0x02);                     // 透過 guard 存取資料
// guard 離開作用範圍 → 自動解鎖，沒有 unlock() 可以呼叫
```

C++ 的 `lock_guard`、Python 的 `with` 跟 Rust 的觀念一樣都是 **RAII**。差別是在 C++ 和 Python 裡這是**建議的寫法**，在 Rust 裡是**唯一的寫法**。

| | C++ / Python / Go | Rust |
|---|---|---|
| 解鎖 | 可以手動 `unlock()`，或用 guard / `with` / `defer` | 只能靠 guard 被 drop 時自動解鎖 |
| 解鎖時間點 | Go 的 `defer` 要等**整個函式結束** | guard **離開作用範圍**就解鎖，可以用 `{ }` 控制 |
| 資料放哪裡 | 和鎖分開，沒上鎖也能直接改 | 資料在 Mutex **裡面**，沒上鎖就**拿不到** |
| 解鎖後還能用資料嗎 | 可以，編譯器不會阻止（這就是 bug） | 不行，**編譯器直接報錯** |
| 忘記上鎖 | 可以編譯。Go 要用 `go run -race` 在執行時才抓得到 | **編譯不過** |

**「忘記上鎖」和「解鎖後還在用資料」這兩種常見的 bug，在 Rust 裡根本編譯不過。**

---

## 提早解鎖

**做法 1：`drop()`**
```rust
let mut guard = value.lock().await;
guard.push(0x02);
drop(guard);            // 在這裡就解鎖
do_something_slow().await;
```

**做法 2：用 `{ }` 包起來**（`gatt_server_cb.rs` 第 88–101 行）
```rust
loop {
    {                                         // ← 多一層大括號
        let mut value = value.lock().await;   // 上鎖
        notifier.notify(value.to_vec()).await;
        for v in &mut *value { *v = v.saturating_sub(1); }
    }                                         // ← 在這裡自動解鎖
    sleep(Duration::from_secs(5)).await;      // 等待時鎖已經放開了
}
```

沒有這層大括號的話，**睡 5 秒時還拿著鎖**，這段期間手機來 read 或 write，callback 都要卡住等。這層看似多餘的大括號，就是 Rust 版的 `unlock()`。

---

## 另一種做法：Channel

Go 的名言：*"Don't communicate by sharing memory; share memory by communicating."*

Rust 的 `tokio::sync::mpsc` 用法跟 Go 的 channel 很像：

```rust
let (tx, mut rx) = tokio::sync::mpsc::channel(8);   // 類似 Go 的 make(chan T, 8)

tx.send(config).await;                               // 類似 ch <- config
while let Some(cfg) = rx.recv().await { ... }        // 類似 for cfg := range ch
```

應用在網路配置功能上的例子，請看 [05-wifi-provisioning-design.md](05-wifi-provisioning-design.md)。
