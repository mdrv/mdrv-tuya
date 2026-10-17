# mdrv-tuya

Self-hosted BLE daemon and live web UI for Tuya-based smart lamps (in-lite
INSM044), a local alternative to the cloud app. A Rust daemon owns the
Bluetooth link and speaks Tuya's encrypted GATT protocol. A Svelte 5 frontend
is embedded into the binary at build time and gives app-like live control with
sliders, a colour wheel and presets, straight from the browser. No cloud, no
Tuya account, no phone app required, though the app keeps working whenever the
daemon is idle.

> [!CAUTION]
> **Personal software, maintained for the author only.** The repo is public so
> the release pipeline has a home, not because it is a supported product.
> Development is heavily LLM-assisted: most commits are written with AI agents
> and verified only against the author's own lamps. Expect breaking changes at
> any bump, no support, no stability guarantee, no release schedule.

> Requires: BlueZ and a LE-capable Bluetooth adapter, `bun`, `just`.
> Keys come from `~/.config/mdrv-tuya/lamps.toml`. How to get them is
> documented in `/x/m/v270/tuya/04-cloud-keys.md`.

## Quick start

```
just run                          # build frontend + binary, start daemon
```

Then open http://127.0.0.1:4141. On a phone, run with `--hostname 0.0.0.0`
and open `http://<pc-ip>:4141`.

## Compatibility

Written for the in-lite INSM044 and tested only on it (two bulbs, protocol
v3). Any Tuya BLE bulb that speaks protocol v3 over service `0x1910` and uses
the same datapoint map (switch 1, mode 2, brightness 3, temperature 4, colour
5) should work with just a `lamps.toml` entry, but no other model has been
tested. Wi-Fi-only Tuya devices are out of scope entirely.

## Development

| Command             | What it does                                          |
| ------------------- | ----------------------------------------------------- |
| `just build`        | frontend (bun) + release build of the daemon          |
| `just run`          | build + run (extra args pass through, e.g. `--debug`) |
| `just dev-frontend` | vite dev server on :5276, proxies /ws to :4141        |
| `just check`        | cargo check + frontend type check                     |
| `just test`         | unit tests (Rust + frontend)                          |

## Features

- **Live control.** Power, brightness, white temperature, colour (hue, sat,
  val) with roughly 1 s round-trip over one held BLE connection per lamp.
- **Lamp selector.** Focus a single lamp or use the all-lamps view with
  global presets and all-on/all-off. The choice persists across reloads.
- **Presets.** Named looks in `lamps.toml` (`[preset.sleep]`, …), one tap to
  apply per lamp or to every lamp at once, editable without reloading.
  Chips show a colour swatch and reveal exact values on hover.
- **Countdown timer.** Device-side auto-off (15/30/60 min or custom via the
  CLI). The timer lives in the bulb, so it survives daemon restarts.
- **Kelvin readout.** The temperature slider shows its approximate colour
  temperature next to the label.
- **CLI one-shots.** `mdrv-tuya --exec A off`, `--exec A bright 500`,
  `--exec A preset sleep`, `--exec A countdown 900` for scripts and
  keybinds, without opening the UI. `--scan` lists nearby Tuya BLE lamps
  with their decrypted identity.
- **Hot reload.** `kill -HUP <pid>` re-reads `lamps.toml`; lamp, credential
  and preset changes apply without restarting the daemon.
- **Failure toasts.** Confirmed changes pop a green toast, failures a red
  one with the lamp's name; the server logs every command, retry and dead
  session.
- **App coexistence.** The daemon holds a lamp's connection only while in use
  and disconnects after `--idle-timeout` (default 300 s), which frees the bulb
  for the Tuya app.
- **Embedded UI.** The frontend is a single `index.html` compiled into the
  binary. The daemon is one file with no static dir to deploy.
- **Protocol v3.** AES-128-CBC framing, login and session keys derived from
  the cloud `local_key`. Writes are retried with fresh sequence numbers when
  the radio silently drops them.

## Testing

Offline checks:

```
just check        # types: cargo check + svelte-check
just test         # 24 Rust tests + 16 frontend tests
```

The Rust tests cover the protocol framing (CRC, varints, AES frame
round-trip, MTU fragmenting), datapoint parsing, config parsing, the write
builders (including the ordering rule that brightness must precede
mode/colour writes) and the CLI command parser. The frontend tests cover
the colour math behind the hue ring, the Kelvin mapping and the preset
swatches.

Live checklist, in order (needs a lamp in radio range):

1. `just run --debug`, open the UI. The status pill goes Live and the card
   shows the lamp's real values within about 10 s (the first wake runs a BLE
   discovery).
2. Per lamp: toggle power, drag brightness, drag temperature, drag the hue
   ring, tap every preset. The card follows the bulb's pushed state, and the
   mode tab follows the bulb (a colour write selects the Colour tab).
3. Power the lamp off at the switch mid-session. The card goes offline; the
   next command wakes it again.
4. Leave the daemon idle past `--idle-timeout`. It drops the BLE link, and
   the Tuya app can take the bulb.
5. Restart the daemon. The UI reconnects on its own.
6. On a weak link (one lamp here sits at −80 dBm), watch the `--debug` log
   for ACK timeouts and retries; commands still land.
7. Release smoke: `./target/release/mdrv-tuya --version` prints the version
   from `Cargo.toml`.

## What mdrv-tuya is not

- Not a Tuya cloud client. Everything happens over BLE; nothing is sent to
  the internet.
- Not a general Tuya gateway. Only BLE protocol-v3 bulbs with the standard
  `0x1910` service are supported, so far only the INSM044.
- Not multi-user. There is no auth; bind to 127.0.0.1 unless your LAN is
  trusted.
