//! Tuya BLE protocol v3 client — Rust port of the verified `tuya_ctl.py`
//! (protocol reference: `/x/m/v270/tuya/05-protocol.md`).
//!
//! One [`lamp_task`] per lamp. The task idles until a command arrives, then
//! connects + handshakes (DEVICE_INFO → PAIR → STATUS), holds the session for
//! further commands, and disconnects after `idle_timeout` so the phone app can
//! use the bulb's single BLE slot.

use aes::cipher::{block_padding::NoPadding, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use anyhow::{anyhow, bail, Context, Result};
use bluer::gatt::remote::{Characteristic, CharacteristicWriteRequest};
use cbc::{Decryptor, Encryptor};
use futures_util::StreamExt;
use md5::{Digest, Md5};
use rand::random;
use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

const SERVICE: Uuid = Uuid::from_u128(0x0000_1910_0000_1000_8000_0080_5f9b_34fb);
const CHAR_NOTIFY: Uuid = Uuid::from_u128(0x0000_2b10_0000_1000_8000_0080_5f9b_34fb);
const CHAR_WRITE: Uuid = Uuid::from_u128(0x0000_2b11_0000_1000_8000_0080_5f9b_34fb);
const GATT_MTU: usize = 20;
const ACK_TIMEOUT: Duration = Duration::from_secs(8);
const WRITE_ATTEMPTS: u8 = 3;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
/// The bulb pushes the DP echo of a write seconds AFTER acking it; after a
/// command batch we drain pushes until this much silence so UI state lags
/// at most ~one window instead of one command.
const SETTLE_WINDOW: Duration = Duration::from_millis(1200);

type Aes128CbcEnc = Encryptor<aes::Aes128>;
type Aes128CbcDec = Decryptor<aes::Aes128>;

/// Credentials for one lamp, from `lamps.toml`.
#[derive(Clone)]
pub struct LampCreds {
    pub name: String,
    pub mac: String,
    pub device_id: String,
    pub uuid: String,
    pub local_key: String,
}

/// One datapoint write on the wire: `id · type · len · value`.
#[derive(Clone, Debug)]
pub struct DpWrite {
    pub id: u8,
    pub dtype: u8,
    pub value: Vec<u8>,
}

impl DpWrite {
    pub fn bool(id: u8, on: bool) -> Self {
        Self {
            id,
            dtype: 1,
            value: vec![u8::from(on)],
        }
    }

    pub fn value(id: u8, v: u32) -> Self {
        Self {
            id,
            dtype: 2,
            value: (v as i32).to_be_bytes().to_vec(),
        }
    }

    pub fn enum_val(id: u8, v: u8) -> Self {
        Self {
            id,
            dtype: 4,
            value: vec![v],
        }
    }

    /// Colour string is ascii hex `hhhhssssvvvv` (hue 0–360, sat/val 0–1000).
    pub fn colour(h: u32, s: u32, v: u32) -> Self {
        Self {
            id: 5,
            dtype: 3,
            value: format!("{h:04x}{s:04x}{v:04x}").into_bytes(),
        }
    }
}

/// A command from the app layer to one lamp.
#[derive(Clone, Debug)]
pub enum LampCmd {
    Write(Vec<DpWrite>),
    Status,
}

/// Full lamp state as read from DP pushes (v3 wire format).
#[derive(Clone, Debug, Default)]
pub struct DpState {
    pub on: Option<bool>,
    /// 0 white · 1 colour · 2 scene
    pub mode: Option<u8>,
    pub brightness: Option<u32>,
    pub temp: Option<u32>,
    pub hue: Option<u32>,
    pub sat: Option<u32>,
    pub val: Option<u32>,
}

impl DpState {
    /// Parse a FUN_RECEIVE_DP payload; returns true if anything changed.
    fn parse_push(&mut self, data: &[u8]) -> bool {
        let mut dirty = false;
        let mut pos = 0;
        while data.len() - pos >= 4 {
            let (id, dtype, dlen) = (data[pos], data[pos + 1], data[pos + 2] as usize);
            let Some(val) = data.get(pos + 3..pos + 3 + dlen) else {
                break;
            };
            pos += 3 + dlen;
            match (id, dtype) {
                (1, 1) => set(&mut self.on, val.first().map(|b| *b != 0), &mut dirty),
                (2, 4) => set(&mut self.mode, val.first().copied(), &mut dirty),
                (3, 2) => set(
                    &mut self.brightness,
                    be_i32(val).map(|v| v.max(0) as u32),
                    &mut dirty,
                ),
                (4, 2) => set(
                    &mut self.temp,
                    be_i32(val).map(|v| v.max(0) as u32),
                    &mut dirty,
                ),
                (5, 3) => {
                    if let Ok(s) = std::str::from_utf8(val) {
                        let part = |r: &str| u32::from_str_radix(r, 16).unwrap_or(0);
                        set(&mut self.hue, s.get(0..4).map(part), &mut dirty);
                        set(&mut self.sat, s.get(4..8).map(part), &mut dirty);
                        set(&mut self.val, s.get(8..12).map(part), &mut dirty);
                    }
                }
                _ => {}
            }
        }
        dirty
    }
}

fn set<T: PartialEq>(slot: &mut Option<T>, value: Option<T>, dirty: &mut bool) {
    if value.is_some() && slot.as_ref() != value.as_ref() {
        *slot = value;
        *dirty = true;
    }
}

fn be_i32(val: &[u8]) -> Option<i32> {
    Some(i32::from_be_bytes(val.try_into().ok()?))
}

/// Events flowing from the session task to the app layer.
#[derive(Clone, Debug)]
pub enum SessionEvent {
    Connected,
    Disconnected(String),
    State(DpState),
}

fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            let lsb = crc & 1;
            crc >>= 1;
            if lsb != 0 {
                crc ^= 0xA001;
            }
        }
    }
    crc
}

fn pack_varint(mut v: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(3);
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            return out;
        }
    }
}

fn unpack_varint(data: &[u8], pos: usize) -> Option<(usize, usize)> {
    let mut result = 0usize;
    for off in 0..5 {
        let b = *data.get(pos + off)?;
        result |= usize::from(b & 0x7f) << (off * 7);
        if b & 0x80 == 0 {
            return Some((result, pos + off + 1));
        }
    }
    None
}

/// Encrypt one protocol frame: `flag · iv · AES-CBC(seq·resp·code·len·data·crc·pad)`.
fn frame(key: &[u8; 16], seq: u32, code: u16, data: &[u8], response_to: u32) -> Vec<u8> {
    let mut raw = Vec::with_capacity(12 + data.len() + 18);
    raw.extend_from_slice(&seq.to_be_bytes());
    raw.extend_from_slice(&response_to.to_be_bytes());
    raw.extend_from_slice(&code.to_be_bytes());
    raw.extend_from_slice(&(data.len() as u16).to_be_bytes());
    raw.extend_from_slice(data);
    raw.extend_from_slice(&crc16(&raw).to_be_bytes());
    raw.resize(raw.len().next_multiple_of(16), 0);

    let iv: [u8; 16] = random();
    let mut out = Vec::with_capacity(17 + raw.len());
    // code 0 (DEVICE_INFO) uses the login key, everything else the session
    // key — the flag byte tells the peer which one we used.
    out.push(if code == 0 { 0x04 } else { 0x05 });
    out.extend_from_slice(&iv);
    out.extend_from_slice(
        &Aes128CbcEnc::new(key.into(), (&iv).into()).encrypt_padded_vec_mut::<NoPadding>(&raw),
    );
    out
}

/// Split an encrypted frame into ≤20-byte GATT writes (varint pkt#, header on pkt 0).
fn fragment(enc: &[u8], protocol_version: u8) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    let mut pos = 0;
    let mut num = 0usize;
    while pos < enc.len() {
        let mut pkt = pack_varint(num);
        if num == 0 {
            pkt.extend(pack_varint(enc.len()));
            pkt.push(protocol_version << 4);
        }
        let take = (GATT_MTU - pkt.len()).min(enc.len() - pos);
        pkt.extend_from_slice(&enc[pos..pos + take]);
        packets.push(pkt);
        pos += take;
        num += 1;
    }
    packets
}

/// Timezone in centihours (e.g. +07:00 → 700), probed once via `date +%z`.
fn tz_centihours() -> i16 {
    static TZ: OnceLock<i16> = OnceLock::new();
    *TZ.get_or_init(|| {
        std::process::Command::new("date")
            .arg("%z")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| {
                let s = s.trim();
                let sign = *s.as_bytes().first()?;
                let h: i16 = s.get(1..3)?.parse().ok()?;
                let m: i16 = s.get(3..5)?.parse().ok()?;
                Some(if sign == b'-' {
                    -(h * 100 + m)
                } else {
                    h * 100 + m
                })
            })
            .unwrap_or(0)
    })
}

/// Mutable protocol state for one lamp session.
struct Session {
    device: bluer::Device,
    local_key: [u8; 6],
    login_key: [u8; 16],
    session_key: Option<[u8; 16]>,
    protocol_version: u8,
    seq: u32,
    write_char: Characteristic,
    /// Seq → ACK sender. Callers keep the matching receiver.
    pending: HashMap<u32, oneshot::Sender<u8>>,
    buf: Vec<u8>,
    buf_len: usize,
    next_pkt: usize,
}

impl Session {
    fn new(creds: LampCreds, device: bluer::Device, write_char: Characteristic) -> Self {
        let key6: [u8; 6] = creds.local_key.as_bytes()[..6]
            .try_into()
            .expect("key >= 6 chars");
        let mut hasher = Md5::new();
        hasher.update(key6);
        let login_key: [u8; 16] = hasher.finalize().into();
        Self {
            device,
            local_key: key6,
            login_key,
            session_key: None,
            protocol_version: 2,
            seq: 0,
            write_char,
            pending: HashMap::new(),
            buf: Vec::new(),
            buf_len: 0,
            next_pkt: 0,
        }
    }

    fn key_for(&self, code: u16) -> Result<[u8; 16]> {
        if code == 0 {
            return Ok(self.login_key);
        }
        self.session_key
            .ok_or_else(|| anyhow!("session key not established (code {code:#06x})"))
    }

    /// Build + write a frame. When `wait`, the ACK sender is stored under the
    /// new seq and the receiver returned to the caller.
    async fn send(
        &mut self,
        code: u16,
        data: &[u8],
        response_to: u32,
        wait: bool,
    ) -> Result<(u32, Option<oneshot::Receiver<u8>>)> {
        let key = self.key_for(code)?;
        self.seq += 1;
        let seq = self.seq;
        let waiter = if wait {
            let (tx, rx) = oneshot::channel();
            self.pending.insert(seq, tx);
            Some(rx)
        } else {
            None
        };
        let enc = frame(&key, seq, code, data, response_to);
        for pkt in fragment(&enc, self.protocol_version) {
            self.write_char
                .write_ext(&pkt, &CharacteristicWriteRequest::default())
                .await
                .context("GATT write")?;
        }
        Ok((seq, waiter))
    }

    /// Reassemble notification packets; returns a complete frame when done.
    fn accept_packet(&mut self, data: &[u8]) -> Option<Vec<u8>> {
        let (num, mut pos) = unpack_varint(data, 0)?;
        if num != self.next_pkt {
            tracing::warn!(got = num, expected = self.next_pkt, "packet gap, resyncing");
            self.buf.clear();
            self.next_pkt = 0;
            return None;
        }
        if num == 0 {
            let (len, p) = unpack_varint(data, pos)?;
            self.buf_len = len;
            pos = p + 1; // skip protocol-version byte
            self.buf = Vec::with_capacity(self.buf_len);
        }
        self.buf.extend_from_slice(&data[pos..]);
        self.next_pkt += 1;
        if self.buf.len() < self.buf_len {
            return None;
        }
        self.next_pkt = 0;
        Some(std::mem::take(&mut self.buf))
    }

    /// Decrypt + dispatch one complete frame.
    fn handle_frame(&mut self, buf: &[u8], dp: &mut DpState) -> Result<Vec<Job>> {
        let flag = *buf.first().ok_or_else(|| anyhow!("empty frame"))?;
        let key = match flag {
            4 => self.login_key,
            5 => self
                .session_key
                .ok_or_else(|| anyhow!("frame flag 05 before session key"))?,
            other => bail!("unhandled security flag {other}"),
        };
        if buf.len() < 17 {
            bail!("frame shorter than flag+iv");
        }
        let plain = Aes128CbcDec::new((&key).into(), (&buf[1..17]).into())
            .decrypt_padded_vec_mut::<NoPadding>(&buf[17..])
            .map_err(|e| anyhow!("decrypt: {e}"))?;

        let seq = u32::from_be_bytes(plain[0..4].try_into().expect("len 4"));
        let resp_to = u32::from_be_bytes(plain[4..8].try_into().expect("len 4"));
        let code = u16::from_be_bytes(plain[8..10].try_into().expect("len 2"));
        let length = u16::from_be_bytes(plain[10..12].try_into().expect("len 2")) as usize;
        let data = plain
            .get(12..12 + length)
            .ok_or_else(|| anyhow!("frame shorter than declared length"))?;

        tracing::debug!(code = format!("{code:#06x}"), seq, resp_to, "frame in");
        match code {
            0 => {
                // DEVICE_INFO: [2] proto major · [6..12] srand → session key
                if data.len() < 46 {
                    bail!("DEVICE_INFO response too short");
                }
                self.protocol_version = data[2];
                let mut hasher = Md5::new();
                hasher.update(self.local_key);
                hasher.update(&data[6..12]);
                self.session_key = Some(hasher.finalize().into());
                self.resolve(resp_to, 0);
                Ok(Vec::new())
            }
            1 => {
                // PAIR result: 0 ok · 2 already paired (our success path)
                let mut result = data.first().copied().unwrap_or(1);
                if result == 2 {
                    tracing::debug!("already paired (cloud-bound bulb) — ok");
                    result = 0;
                }
                self.resolve(resp_to, result);
                Ok(Vec::new())
            }
            3 => {
                self.resolve(resp_to, data.first().copied().unwrap_or(0));
                Ok(Vec::new())
            }
            0x8001 | 0x8003 | 0x8004 => {
                dp.parse_push(data);
                Ok(vec![Job::AckPush { code, resp_to: seq }])
            }
            0x8011 | 0x8012 => Ok(vec![Job::Time1 { code, resp_to: seq }]),
            other => {
                if resp_to != 0 {
                    // generic ACK — must always resolve its waiter (scar!)
                    self.resolve(resp_to, data.first().copied().unwrap_or(0));
                } else {
                    tracing::debug!(code = format!("{other:#06x}"), "unhandled device frame");
                }
                Ok(Vec::new())
            }
        }
    }

    fn resolve(&mut self, resp_to: u32, result: u8) {
        if let Some(tx) = self.pending.remove(&resp_to) {
            let _ = tx.send(result);
        }
    }

    async fn disconnect(mut self) {
        self.pending.clear();
        if let Err(e) = self.device.disconnect().await {
            tracing::warn!(error = %e, "disconnect failed");
        }
    }
}

enum Job {
    /// The device pushed datapoints; acknowledge with the same code.
    AckPush { code: u16, resp_to: u32 },
    /// Device wants the wall clock (TIME1/TIME2 → ms + centihours answer).
    Time1 { code: u16, resp_to: u32 },
}

async fn execute_job(sess: &mut Session, job: Job) -> Result<()> {
    match job {
        Job::AckPush { code, resp_to } => {
            sess.send(code, b"", resp_to, false).await?;
        }
        Job::Time1 { code, resp_to } => {
            let ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            let mut data = ms.to_string().into_bytes();
            data.extend_from_slice(&tz_centihours().to_be_bytes());
            sess.send(code, &data, resp_to, false).await?;
        }
    }
    Ok(())
}

async fn emit_state(events: &mpsc::Sender<SessionEvent>, dp: &DpState) {
    let _ = events.send(SessionEvent::State(dp.clone())).await;
}

/// Handle one notification packet end-to-end: reassemble, decrypt/dispatch,
/// run produced jobs, and publish a state snapshot if datapoints moved.
async fn process_packet(
    sess: &mut Session,
    pkt: Vec<u8>,
    dp: &mut DpState,
    events: &mpsc::Sender<SessionEvent>,
) -> Result<()> {
    let Some(frame) = sess.accept_packet(&pkt) else {
        return Ok(());
    };
    for job in sess.handle_frame(&frame, dp)? {
        execute_job(sess, job).await?;
    }
    emit_state(events, dp).await;
    Ok(())
}

/// Pump decrypted frames into `jobs`, emitting state snapshots when DPs move.
async fn pump_frames(
    sess: &mut Session,
    frame_rx: &mut mpsc::Receiver<Vec<u8>>,
    dp: &mut DpState,
    events: &mpsc::Sender<SessionEvent>,
) -> Result<()> {
    let Some(pkt) = frame_rx.recv().await else {
        bail!("notification stream ended");
    };
    process_packet(sess, pkt, dp, events).await
}

/// Wait until the waiter for `seq` resolves, pumping frames while waiting.
/// Cancel-safe: the oneshot receiver can be polled across select iterations.
async fn wait_ack(
    sess: &mut Session,
    mut waiter: oneshot::Receiver<u8>,
    seq: u32,
    frame_rx: &mut mpsc::Receiver<Vec<u8>>,
    dp: &mut DpState,
    events: &mpsc::Sender<SessionEvent>,
    timeout: Duration,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        tokio::select! {
            res = &mut waiter => {
                return match res {
                    Ok(0) => {
                        emit_state(events, dp).await;
                        Ok(())
                    }
                    Ok(other) => Err(anyhow!("device rejected seq {seq}: result {other}")),
                    Err(_) => Err(anyhow!("ack channel dropped for seq {seq}")),
                };
            }
            // Frames keep flowing while we wait (DP pushes, time requests).
            // On error we surface it and let the caller retry the write.
            res = pump_frames(sess, frame_rx, dp, events) => {
                res?;
            }
            _ = tokio::time::sleep_until(deadline) => {
                sess.pending.remove(&seq);
                bail!("no ACK for seq {seq}");
            }
        }
    }
}

/// Write one DP with retry: weak RF links silently swallow GATT writes, so a
/// missing ACK gets a fresh-seq resend (see AGENTS.md scars).
async fn write_dp(
    sess: &mut Session,
    w: &DpWrite,
    frame_rx: &mut mpsc::Receiver<Vec<u8>>,
    dp: &mut DpState,
    events: &mpsc::Sender<SessionEvent>,
) {
    let mut payload = vec![w.id, w.dtype, w.value.len() as u8];
    payload.extend_from_slice(&w.value);
    for attempt in 1..=WRITE_ATTEMPTS {
        let (seq, waiter) = match sess.send(2, &payload, 0, true).await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(attempt, dp = w.id, error = %e, "write failed");
                return;
            }
        };
        let Some(waiter) = waiter else { return };
        match wait_ack(sess, waiter, seq, frame_rx, dp, events, ACK_TIMEOUT).await {
            Ok(()) => return,
            Err(e) => {
                tracing::warn!(attempt, dp = w.id, error = %e, "write not acked, retrying");
            }
        }
    }
    tracing::error!(dp = w.id, "write dropped after all attempts");
}

/// BlueZ only knows devices it has seen; if a direct connect fails, run a
/// short discovery and retry.
async fn ensure_device(adapter: &bluer::Adapter, addr: bluer::Address) -> Result<bluer::Device> {
    let device = adapter.device(addr)?;
    if device.is_connected().await.unwrap_or(false) {
        return Ok(device);
    }
    match device.connect().await {
        Ok(()) => return Ok(device),
        Err(e) => {
            tracing::debug!(%addr, error = %e, "direct connect failed, discovering");
        }
    }
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut discovery = adapter
        .discover_devices()
        .await
        .context("adapter discovery")?;
    while Instant::now() < deadline {
        tokio::select! {
            ev = discovery.next() => match ev {
                Some(bluer::AdapterEvent::DeviceAdded(address)) if address == addr => break,
                Some(_) => {}
                None => break,
            },
            _ = tokio::time::sleep(Duration::from_millis(200)) => {}
        }
    }
    drop(discovery);
    device
        .connect()
        .await
        .context("BLE connect after discovery")?;
    Ok(device)
}

async fn find_chars(device: &bluer::Device) -> Result<(Characteristic, Characteristic)> {
    for svc in device.services().await.context("services")? {
        if svc.uuid().await? != SERVICE {
            continue;
        }
        let mut write: Option<Characteristic> = None;
        let mut notify: Option<Characteristic> = None;
        for ch in svc.characteristics().await.context("characteristics")? {
            let u = ch.uuid().await.context("characteristic uuid")?;
            if u == CHAR_WRITE {
                write = Some(ch);
            } else if u == CHAR_NOTIFY {
                notify = Some(ch);
            }
        }
        if let (Some(w), Some(n)) = (write, notify) {
            return Ok((w, n));
        }
    }
    bail!("Tuya service {SERVICE} with 0x2B10/0x2B11 not found — is this an INSM044?")
}

/// Connect (with discovery fallback), then DEVICE_INFO → PAIR → STATUS.
/// Returns the ready session + the receiver of the notification pump.
async fn connect_and_handshake(
    adapter: &bluer::Adapter,
    creds: &LampCreds,
    dp: &mut DpState,
    events: &mpsc::Sender<SessionEvent>,
) -> Result<(Session, mpsc::Receiver<Vec<u8>>)> {
    let addr: bluer::Address = creds.mac.parse().context("bad MAC in config")?;
    let device = ensure_device(adapter, addr).await.context("BLE connect")?;
    let (write_char, notify_char) = find_chars(&device).await?;

    // Pipe raw notifications into a channel so every phase can pump frames.
    let (frame_tx, mut frame_rx) = mpsc::channel(32);
    let notifier = notify_char.notify().await.context("subscribe notify")?;
    tokio::spawn(async move {
        tokio::pin!(notifier);
        while let Some(v) = notifier.next().await {
            if frame_tx.send(v).await.is_err() {
                break;
            }
        }
    });

    let mut sess = Session::new(creds.clone(), device, write_char);

    // DEVICE_INFO (login key) → establishes the session key
    let (seq, waiter) = sess.send(0, b"", 0, true).await?;
    let waiter = waiter.expect("DEVICE_INFO registered a waiter");
    wait_ack(
        &mut sess,
        waiter,
        seq,
        &mut frame_rx,
        dp,
        events,
        HANDSHAKE_TIMEOUT,
    )
    .await?;

    // PAIR: uuid + local_key[:6] + device_id, zero-padded to 44 bytes
    let mut req = Vec::with_capacity(44);
    req.extend_from_slice(creds.uuid.as_bytes());
    req.extend_from_slice(&sess.local_key);
    req.extend_from_slice(creds.device_id.as_bytes());
    req.resize(44, 0);
    let (seq, waiter) = sess.send(1, &req, 0, true).await?;
    let waiter = waiter.expect("PAIR registered a waiter");
    wait_ack(
        &mut sess,
        waiter,
        seq,
        &mut frame_rx,
        dp,
        events,
        HANDSHAKE_TIMEOUT,
    )
    .await?;

    // STATUS: triggers a full DP push (the ACK and the push both arrive)
    let (seq, waiter) = sess.send(3, b"", 0, true).await?;
    let waiter = waiter.expect("STATUS registered a waiter");
    wait_ack(
        &mut sess,
        waiter,
        seq,
        &mut frame_rx,
        dp,
        events,
        HANDSHAKE_TIMEOUT,
    )
    .await?;

    tracing::info!(lamp = %creds.name, protocol = sess.protocol_version, "connected");
    Ok((sess, frame_rx))
}

/// Long-lived task for one lamp: sleeps until a command arrives, connects,
/// serves commands, disconnects on idle; reconnects on the next command.
pub async fn lamp_task(
    creds: LampCreds,
    idle_timeout: Duration,
    adapter_name: Option<String>,
    mut rx: mpsc::Receiver<LampCmd>,
    events: mpsc::Sender<SessionEvent>,
) {
    let session = match bluer::Session::new().await.context("BlueZ session") {
        Ok(s) => s,
        Err(e) => {
            let _ = events
                .send(SessionEvent::Disconnected(format!("BlueZ session: {e}")))
                .await;
            return;
        }
    };
    let adapter = match resolve_adapter(&session, adapter_name).await {
        Ok(a) => a,
        Err(e) => {
            let _ = events
                .send(SessionEvent::Disconnected(format!("adapter: {e}")))
                .await;
            return;
        }
    };

    'reconnect: loop {
        // Phase 1 — idle: wait for a command that justifies taking the BLE slot.
        let Some(first) = rx.recv().await else {
            break 'reconnect; // app shutting down
        };
        let mut queue: VecDeque<LampCmd> = VecDeque::new();
        queue.push_back(first);
        while let Ok(more) = rx.try_recv() {
            queue.push_back(more);
        }
        let mut dp = DpState::default();

        // Phase 2 — connect + handshake (frames pumped inside wait_ack).
        let (mut sess, mut frame_rx) =
            match connect_and_handshake(&adapter, &creds, &mut dp, &events).await {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::warn!(lamp = %creds.name, error = %e, "connect failed");
                    let _ = events
                        .send(SessionEvent::Disconnected(format!("connect: {e}")))
                        .await;
                    continue 'reconnect;
                }
            };
        let _ = events.send(SessionEvent::Connected).await;
        emit_state(&events, &dp).await;

        // Phase 3 — ready: serve commands until idle.
        let idle = tokio::time::sleep(idle_timeout);
        tokio::pin!(idle);
        'session: loop {
            tokio::select! {
                cmd = rx.recv() => {
                    let Some(cmd) = cmd else { break 'reconnect };
                    idle.as_mut().reset(tokio::time::Instant::now() + idle_timeout);
                    queue.push_back(cmd);
                    while let Some(q) = queue.pop_front() {
                        match q {
                            LampCmd::Write(writes) => {
                                for w in writes {
                                    write_dp(&mut sess, &w, &mut frame_rx, &mut dp, &events).await;
                                }
                            }
                            LampCmd::Status => {
                                if let Ok((seq, Some(waiter))) = sess.send(3, b"", 0, true).await {
                                    let _ = wait_ack(
                                        &mut sess, waiter, seq, &mut frame_rx, &mut dp, &events,
                                        ACK_TIMEOUT,
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                    // Settle: keep draining notifications until the bulb goes
                    // quiet, so its (late) DP echoes reach the UI promptly.
                    while let Ok(pkt) =
                        tokio::time::timeout(SETTLE_WINDOW, frame_rx.recv()).await
                    {
                        let Some(pkt) = pkt else { break };
                        if process_packet(&mut sess, pkt, &mut dp, &events)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                _ = &mut idle => {
                    tracing::info!(lamp = %creds.name, "idle timeout, disconnecting");
                    break 'session;
                }
            }
        }
        sess.disconnect().await;
        let _ = events.send(SessionEvent::Disconnected("idle".into())).await;
        continue 'reconnect;
    }
}

async fn resolve_adapter(session: &bluer::Session, name: Option<String>) -> Result<bluer::Adapter> {
    match name {
        Some(n) => session.adapter(&n).context(format!("adapter {n}")),
        None => session.default_adapter().await.context("default adapter"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc16_modbus_known_vector() {
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    #[test]
    fn varint_roundtrip() {
        for v in [0usize, 1, 127, 128, 300, 16383, 16384, 1 << 20] {
            let enc = pack_varint(v);
            assert_eq!(unpack_varint(&enc, 0), Some((v, enc.len())));
        }
    }

    #[test]
    fn frame_roundtrip() {
        let key = [7u8; 16];
        let data = b"hello tuya";
        let enc = frame(&key, 0xDEAD_BEEF, 0x0002, data, 0x1234);
        assert_eq!(enc[0], 0x05); // session flag
        let plain = Aes128CbcDec::new((&key).into(), (&enc[1..17]).into())
            .decrypt_padded_vec_mut::<NoPadding>(&enc[17..])
            .unwrap();
        assert_eq!(
            u32::from_be_bytes(plain[0..4].try_into().unwrap()),
            0xDEAD_BEEF
        );
        assert_eq!(u32::from_be_bytes(plain[4..8].try_into().unwrap()), 0x1234);
        assert_eq!(u16::from_be_bytes(plain[8..10].try_into().unwrap()), 0x0002);
        assert_eq!(
            u16::from_be_bytes(plain[10..12].try_into().unwrap()),
            data.len() as u16
        );
        assert_eq!(&plain[12..12 + data.len()], data);
    }

    #[test]
    fn fragment_fits_mtu_and_reassembles() {
        let enc = vec![0xABu8; 75];
        let pkts = fragment(&enc, 3);
        assert!(pkts.iter().all(|p| p.len() <= GATT_MTU));
        let mut out = Vec::new();
        let mut sess_len = 0;
        for (i, p) in pkts.iter().enumerate() {
            let (num, mut pos) = unpack_varint(p, 0).unwrap();
            assert_eq!(num, i);
            if num == 0 {
                let (len, np) = unpack_varint(p, pos).unwrap();
                sess_len = len;
                pos = np + 1;
            }
            out.extend_from_slice(&p[pos..]);
        }
        assert_eq!(out.len(), sess_len);
        assert_eq!(out, enc);
    }

    #[test]
    fn dp_state_parses_push() {
        // dp1 bool true · dp2 enum 01 · dp3 value 1000 · dp5 string
        let mut data = vec![1, 1, 1, 1, 2, 4, 1, 1];
        data.extend_from_slice(&[3, 2, 4, 0, 0, 0x03, 0xE8]);
        // colour payload is exactly 12 ASCII hex chars: hue · sat · val
        data.extend_from_slice(b"\x05\x03\x0c000003e803e8");
        let mut dp = DpState::default();
        assert!(dp.parse_push(&data));
        assert_eq!(dp.on, Some(true));
        assert_eq!(dp.mode, Some(1));
        assert_eq!(dp.brightness, Some(1000));
        assert_eq!(dp.hue, Some(0));
        assert_eq!(dp.sat, Some(1000));
        assert_eq!(dp.val, Some(1000));
    }
}
