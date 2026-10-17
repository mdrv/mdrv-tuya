# mdrv-tuya — agent companion

BLE daemon + live web UI for Tuya BLE smart lamps (in-lite INSM044). Rust owns
Bluetooth and the Tuya BLE protocol; a Svelte 5 frontend (embedded into the
binary at build time) gives app-like live control.

## Build / run / test

```
just build          # frontend (bun) + cargo release build
just run            # build + run daemon
just dev-frontend   # vite dev server with /ws proxy to a running daemon
cargo check         # rust only
cargo test          # unit tests (protocol helpers)
```

Requires: `bluetoothd` running, BlueZ, a LE-capable adapter, `bun`.

## Structure

- `src/main.rs` — clap CLI + logging init, entry point
- `src/config.rs` — TOML config (lamps + presets), `~/.config/mdrv-tuya/lamps.toml`
- `src/tuya.rs` — Tuya BLE protocol v3 client (framing, AES, handshake, session task)
- `src/app.rs` — axum router, WS fan-out, HTTP surface, embedded frontend
- `frontend/` — Svelte 5 + Vite + vanilla-extract SPA, built to `frontend/dist/`,
  embedded via `include_str!`

## Design constraints

- **One BLE slot per bulb.** The phone app must keep working; the daemon
  disconnects after `--idle-timeout` (default 300 s) and reconnects on demand.
- **Protocol v3 only.** Bulbs advertise v3 (mfg byte 1); v4 is not implemented.
- **Silent write drops are real** on weak RF: every write is retried with a
  fresh seq on ACK timeout (3 attempts).
- Encryption keys come only from `lamps.toml` (extracted per
  `/x/m/v270/tuya/04-cloud-keys.md`; they rotate on bulb re-pairing).

## Scars (do not rediscover)

- `0x2B10` = notify (device→host), `0x2B11` = write (host→device) — the
  bluetoothctl GATT dump reads the other way.
- The AES login key is `md5(local_key[:6])` — only the **first 6 chars** of the
  16-char cloud key.
- Every frame with `response_to != 0` must resolve its waiter — unhandled ACKs
  hang the sender while the bulb has already applied the write.
- Writing brightness/temp flips work_mode to white; writing colour flips it to
  colour. Expect unrequested DP2 pushes.
- A bulb can silently drop the BLE link while BlueZ still reports it
  connected, so `is_connected()` lies. Treat any write failure as a dead
  session: drop it and let the next command open a fresh one.

## Commits

Conventional commits, `type: lowercase description`, imperative, ≤72 chars.
Author/committer identity: **Umar Alfarouk `<medrivia@gmail.com>`** (set via
repo-local git config; never commit as anything else).
