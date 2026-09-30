# CLAUDE.md

A BLE GATT server (Rust) running on the robot that lets a phone query/configure Wi-Fi over Bluetooth. Bluetooth goes through `bluer` (BlueZ), networking through `nmrs` (NetworkManager). Target platform is Ubuntu 22.04. See `README.md` for the GATT interface and Command format.

## Common commands

Rust is not installed on the host; always run cargo through the container (the image is built automatically on first run):

```bash
scripts/cargo build
scripts/cargo clippy
scripts/cargo test
scripts/cargo fmt
scripts/cargo run        # uses the host's BlueZ / NM via the host D-Bus
```

- After changing `docker/cargo/Dockerfile`: `CARGO_IMAGE_REBUILD=1 scripts/cargo build`
- On a macOS dev machine, BLE / NM features can't actually run; only build, clippy and unit tests.
- For on-device testing, run `cargo run --example gatt_client -- <status|scan|set|disconnect|raw>` on a separate Linux machine.
- `not authorized` errors from NM operations are polkit permission issues; see "NetworkManager permissions" in the README.

## Architecture

- `main.rs`: sets up NM and bluer sessions, advertises, builds the `Application`; pressing enter on stdin exits (dropping the handles removes the service/advertisement).
- `handler.rs`:
  - `read_network_status`: callback-mode read, returns JSON.
  - `serve_available_networks`: IO-mode notify; spawns one task per subscription: rescan → JSON → send in `writer.mtu()`-sized chunks, terminated by `\n`.
  - `write_command`: callback-mode write, only parses/validates; slow operations (NM connect takes up to 30 s) must go to a background task, otherwise the 30 s ATT timeout is exceeded.
- `setting.rs`: `Command` (serde `tag = "cmd"`, `snake_case`, `deny_unknown_fields`) and the `CommandError → ReqError` mapping.
- `scan.rs`: `nm.scan_networks()` doesn't wait for the scan to finish, so it polls the D-Bus `LastScan` property, waiting up to 15 s.
- `wifi.rs`: `switch_network`; an empty password means an open network, otherwise WPA-PSK.
- `config.rs`: UUIDs. **`examples/gatt_client.rs` has a copy of the UUIDs** (the binary crate has no lib.rs), so keep both in sync when changing them.

Current status: `set_wifi` only validates and doesn't call `wifi::switch_network` yet; `disconnect` returns `NotSupported`.

## Conventions

- The runtime is `tokio` `current_thread`.
- Lints: `unsafe_code = "forbid"`, `unused_must_use = "deny"`, clippy `all` + `pedantic` (warn). Run `scripts/cargo clippy` after changes and make sure there are no new warnings.
- Formatting: `rustfmt.toml` (`max_width = 100`).
- All comments and documentation are in English. Keep them concise.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `docs:`, `build:`, `chore:` …, optional scope); see `.github/prompt/copilot-commit-message-instructions.md`.
- Branching: work on a feature branch and open PRs against `dev`.
