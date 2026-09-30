# SyncAI-Robot-BLE-Server

A BLE GATT server running on the robot (Ubuntu 22.04) that lets a phone query and configure the robot's Wi-Fi over Bluetooth.

- Bluetooth: drives the host's BlueZ via [bluer](https://docs.rs/bluer)
- Networking: drives NetworkManager via [nmrs](https://docs.rs/nmrs)

## GATT interface

Primary service UUID: `12345678-1234-5678-1234-56789abcdef0` (advertised local name is currently `test`)

| Characteristic | UUID | Properties | Content |
|---|---|---|---|
| NetworkStatus | `1234abcd-0001-0000-8000-00805f9b34fb` | read | `{"connected":true,"ssid":"...","signal":-50,"ip":"192.168.1.23"}`; `signal` is RSSI (dBm) |
| AvailableNetworks | `1234abcd-0002-0000-8000-00805f9b34fb` | notify (IO mode) | After subscribing, pushes `[{"ssid":"...","signal":80,"secured":true}, ...]` once per rescan round; `signal` is NM strength (0-100). Split into MTU-sized chunks, terminated by `\n` |
| Command | `1234abcd-0000-0000-8000-00805f9b34fb` | write | JSON command, at most 512 bytes, must be written in a single write (offset must be 0) |

Command format:

```json
{"cmd": "set_wifi", "id": 1, "ssid": "MyWiFi", "password": "secret"}
{"cmd": "disconnect", "id": 2}
```

- `ssid` must be 1-32 bytes and `password` at most 63 bytes; an empty password means an open network.
- Malformed JSON returns `NotSupported`, an oversized command returns `InvalidValueLength`, and an invalid SSID/password returns `Failed`.
- Current status: `set_wifi` is only parsed and validated (it doesn't switch networks yet), and `disconnect` is not implemented yet.

## Project layout

```
src/
  main.rs      Set up NM / BlueZ sessions, advertise, register the GATT application
  config.rs    Service and characteristic UUIDs
  handler.rs   Read / notify / write handlers for the three characteristics
  setting.rs   Command JSON parsing and validation
  scan.rs      Trigger a Wi-Fi rescan and wait for NM's LastScan to update
  wifi.rs      Switch Wi-Fi via NM
  helper.rs    Parse /proc/net/wireless for RSSI
examples/
  gatt_client.rs   GATT client for testing
deploy/polkit/     polkit rule for NetworkManager
docker/cargo/      cargo dev container
docs/              Learning notes (see docs/README.md)
```

## Build (no Rust install needed on the host)

`scripts/cargo` runs cargo inside a container, with the same usage as cargo:

```bash
scripts/cargo build
scripts/cargo clippy
scripts/cargo test
```

There are two ways to run the BLE server; both go through the host's BlueZ (`bluetooth.service`):

```bash
scripts/cargo run                                  # run in the container (host D-Bus socket is mounted)
scripts/cargo build && ./target/debug/SyncAI-Robot-BLE-Server   # run directly on the host
```

- The image (`docker/cargo/Dockerfile`) is built automatically on first run. After changing the Dockerfile, rebuild with `CARGO_IMAGE_REBUILD=1 scripts/cargo build`.
- The image is based on `ubuntu:22.04`, matching the host's glibc, so the built `target/debug/SyncAI-Robot-BLE-Server` runs directly on the host (BLE needs the host's BlueZ).
- The crate download cache lives in the docker volumes `syncai-ble-cargo-registry` / `syncai-ble-cargo-git`.

## NetworkManager permissions (polkit)

The BLE server scans, connects and saves Wi-Fi settings through NetworkManager. NM checks every operation with polkit, which by default only allows users in a **local login session**. Processes started from SSH, a container or systemd don't count as local logins and get:

```
org.freedesktop.NetworkManager.wifi.scan request failed: not authorized
```

This happens whether you run on the host or in the container. Fix it with one of the two options below.

### Option 1: install the polkit rule (recommended for development)

Allows the `syncrobotic` account to perform these three actions without a password:

| polkit action | Purpose |
|---|---|
| `org.freedesktop.NetworkManager.wifi.scan` | rescan (AvailableNetworks) |
| `org.freedesktop.NetworkManager.network-control` | connect / disconnect (SetWifi, Disconnect) |
| `org.freedesktop.NetworkManager.settings.modify.system` | save new Wi-Fi profiles (SetWifi) |

```bash
sudo install -m 644 deploy/polkit/50-syncai-ble-server.pkla /etc/polkit-1/localauthority/50-local.d/
sudo systemctl restart polkit
```

Verify it took effect; all three should show `yes`:

```bash
nmcli general permissions | grep -E 'wifi.scan|network-control|settings.modify.system'
```

Notes:

- Ubuntu 22.04 ships polkit 0.105, which only supports the `.pkla` format. You **cannot** use `/etc/polkit-1/rules.d/*.rules` (the JavaScript format of newer polkit).
- The rule specifies `Identity=unix-user:syncrobotic`. If you run the server as another account, edit `deploy/polkit/50-syncai-ble-server.pkla` and reinstall it.
- **Every process** of that account gets these permissions, not just the BLE server.
- If it still shows `auth` or `no`, check the file is in the right place with `sudo ls -l /etc/polkit-1/localauthority/50-local.d/`, then make sure polkit was restarted.

### Option 2: run as root

NM always allows root, so no polkit rule is needed:

```bash
sudo ./target/debug/SyncAI-Robot-BLE-Server
```

In production, the server is usually run by a systemd service as root or a dedicated account.

## Testing with the GATT client

`examples/gatt_client.rs` must run on a **different** Linux machine with Bluetooth (an adapter can't connect to itself):

```bash
cargo run --example gatt_client -- status              # read NetworkStatus
cargo run --example gatt_client -- scan 3              # subscribe to AvailableNetworks for 3 rounds
cargo run --example gatt_client -- set <ssid> [password]
cargo run --example gatt_client -- disconnect
cargo run --example gatt_client -- raw '<json>'        # send arbitrary data to test error handling
```

By default it scans for a device advertising the primary service UUID; set `BLE_ADDR=AA:BB:CC:DD:EE:FF` to target a specific server.

You can also test directly with the **nRF Connect** app on a phone.
