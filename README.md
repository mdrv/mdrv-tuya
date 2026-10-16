# mdrv-tuya

Self-hosted BLE daemon + live web UI for Tuya-based smart lamps (in-lite INSM044),
a local alternative to the cloud app: a Rust daemon owns the Bluetooth link and
speaks Tuya's encrypted GATT protocol, a Svelte 5 frontend (embedded into the
binary at build time) gives app-like live control — sliders, colour wheel,
presets — straight from the browser. No cloud, no Tuya account, no phone app
required (though the app keeps working whenever the daemon is idle).

> Requires: BlueZ + a LE-capable Bluetooth adapter, `bun`, `just`.
> Keys come from `~/.config/mdrv-tuya/lamps.toml` — how to get them is
> documented in `/x/m/v270/tuya/04-cloud-keys.md`.

## Quick start

```
just run                          # build frontend + binary, start daemon
```

Then open http://127.0.0.1:4141. On a phone, run with `--hostname 0.0.0.0`
and open `http://<pc-ip>:4141`.

## Development

| Command            | What it does                                    |
| ------------------ | ----------------------------------------------- |
| `just build`       | frontend (bun) + release build of the daemon    |
| `just run`         | build + run (extra args pass through, e.g. `--debug`) |
| `just dev-frontend`| vite dev server on :5276, proxies /ws to :4141  |
| `just check`       | cargo check + frontend type check               |
| `just test`        | unit tests (protocol helpers)                   |

## Features

- **Live control** — power, brightness, white temperature, colour (hue/sat/val)
  with ~1 s round-trip over one held BLE connection per lamp.
- **Presets** — named looks in `lamps.toml` (`[preset.sleep]`, …), one tap to
  apply, editable without rebuilding.
- **App coexistence** — the daemon only holds a lamp's connection while in use
  and disconnects after `--idle-timeout` (default 300 s), freeing the bulb for
  the Tuya app.
- **Embedded UI** — the frontend is a single `index.html` compiled into the
  binary; the daemon is one file, no static dir to deploy.
- **Protocol v3** — AES-128-CBC framing, login + session keys derived from the
  cloud `local_key`; writes retried with fresh sequence numbers when the radio
  silently drops them.

## What mdrv-tuya is not

- Not a Tuya cloud client — everything happens over BLE on the LAN segment of
  one; nothing is sent to the internet.
- Not a general Tuya gateway — only BLE protocol-v3 bulbs with the standard
  0x1910 service (INSM044 class) are supported.
- Not multi-user — no auth; bind to 127.0.0.1 unless your LAN is trusted.
