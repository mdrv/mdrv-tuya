//! axum HTTP + WebSocket surface: serves the embedded frontend and fans out
//! live lamp state to all connected UIs (kn26-style WS pattern).

use crate::config::{Config, Lamp, Preset};
use crate::tuya::{DpState, DpWrite, LampCmd, LampCreds, SessionEvent};
use anyhow::{bail, Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

const APP_HTML: &str = include_str!("../frontend/dist/index.html");

/// Serializable snapshot of one lamp for the UI.
#[derive(Clone, Serialize)]
struct LampView {
    name: String,
    connected: bool,
    on: Option<bool>,
    /// 0 white · 1 colour · 2 scene
    mode: Option<u8>,
    brightness: Option<u32>,
    temp: Option<u32>,
    hue: Option<u32>,
    sat: Option<u32>,
    val: Option<u32>,
    countdown: Option<u32>,
}

impl LampView {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            connected: false,
            on: None,
            mode: None,
            brightness: None,
            temp: None,
            hue: None,
            sat: None,
            val: None,
            countdown: None,
        }
    }

    fn apply_dp(&mut self, dp: &DpState) {
        self.on = dp.on;
        self.mode = dp.mode;
        self.brightness = dp.brightness;
        self.temp = dp.temp;
        self.hue = dp.hue;
        self.sat = dp.sat;
        self.val = dp.val;
        self.countdown = dp.countdown;
    }
}

struct AppState {
    /// Config presets — swapped live by SIGHUP reloads.
    presets: Arc<Mutex<BTreeMap<String, Preset>>>,
    states: Arc<Mutex<BTreeMap<String, LampView>>>,
    tx: broadcast::Sender<String>,
    cmds: Arc<Mutex<BTreeMap<String, mpsc::Sender<LampCmd>>>>,
    /// Config-derived credentials per lamp, for reload diffing.
    creds: Arc<Mutex<BTreeMap<String, LampCreds>>>,
    cfg_path: PathBuf,
    idle: Duration,
    adapter: Option<String>,
}

type Shared = Arc<AppState>;

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientMsg {
    Set {
        lamp: String,
        dp: String,
        value: Value,
    },
    Preset {
        lamp: String,
        name: String,
    },
    Refresh {
        lamp: String,
    },
}

impl ClientMsg {
    fn lamp(&self) -> &str {
        match self {
            ClientMsg::Set { lamp, .. }
            | ClientMsg::Preset { lamp, .. }
            | ClientMsg::Refresh { lamp } => lamp,
        }
    }
}

pub async fn serve(
    cfg: Config,
    cfg_path: PathBuf,
    hostname: &str,
    port: u16,
    idle_timeout: u64,
    adapter: Option<String>,
) -> Result<()> {
    let idle = Duration::from_secs(idle_timeout);
    let (tx, _) = broadcast::channel::<String>(64);
    let state: Shared = Arc::new(AppState {
        presets: Arc::new(Mutex::new(cfg.presets.clone())),
        states: Arc::new(Mutex::new(BTreeMap::new())),
        tx,
        cmds: Arc::new(Mutex::new(BTreeMap::new())),
        creds: Arc::new(Mutex::new(BTreeMap::new())),
        cfg_path,
        idle,
        adapter: adapter.clone(),
    });
    for (name, lamp) in &cfg.lamps {
        spawn_lamp(&state, name, lamp);
    }
    // SIGHUP reloads lamps + presets without dropping WS clients.
    tokio::spawn(reload_task(state.clone()));

    let app = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind((hostname, port))
        .await
        .with_context(|| format!("binding {hostname}:{port}"))?;
    tracing::info!(%hostname, port, "listening on http://{hostname}:{port}");
    axum::serve(listener, app).await.context("http serve")
}

/// Shared handles needed to register a lamp task.
struct LampRegistry<'a> {
    idle: Duration,
    adapter: Option<String>,
    tx: &'a broadcast::Sender<String>,
    states: &'a Arc<Mutex<BTreeMap<String, LampView>>>,
}

/// Spawn one lamp's session task + state forwarder and register them.
fn spawn_lamp(state: &Shared, name: &str, lamp: &Lamp) {
    let mut cmds = state.cmds.lock().unwrap();
    let mut creds = state.creds.lock().unwrap();
    let reg = LampRegistry {
        idle: state.idle,
        adapter: state.adapter.clone(),
        tx: &state.tx,
        states: &state.states,
    };
    start_lamp(&reg, &mut cmds, &mut creds, name, lamp);
}

/// Register one lamp while the shared maps are already locked.
fn start_lamp(
    reg: &LampRegistry,
    cmds: &mut BTreeMap<String, mpsc::Sender<LampCmd>>,
    creds: &mut BTreeMap<String, LampCreds>,
    name: &str,
    lamp: &Lamp,
) {
    let creds_l = LampCreds {
        name: name.to_owned(),
        mac: lamp.mac.clone(),
        device_id: lamp.device_id.clone(),
        uuid: lamp.uuid.clone(),
        local_key: lamp.local_key.clone(),
    };
    let (cmd_tx, cmd_rx) = mpsc::channel::<LampCmd>(32);
    let (ev_tx, mut ev_rx) = mpsc::channel::<SessionEvent>(64);
    cmds.insert(name.to_owned(), cmd_tx);
    creds.insert(name.to_owned(), creds_l);
    reg.states
        .lock()
        .unwrap()
        .entry(name.to_owned())
        .or_insert_with(|| LampView::new(name));

    tokio::spawn(crate::tuya::lamp_task(
        LampCreds {
            name: name.to_owned(),
            mac: lamp.mac.clone(),
            device_id: lamp.device_id.clone(),
            uuid: lamp.uuid.clone(),
            local_key: lamp.local_key.clone(),
        },
        reg.idle,
        reg.adapter.clone(),
        cmd_rx,
        ev_tx,
    ));

    let st = reg.states.clone();
    let tx2 = reg.tx.clone();
    let nm = name.to_owned();
    tokio::spawn(async move {
        while let Some(ev) = ev_rx.recv().await {
            {
                let mut guard = st.lock().unwrap();
                let Some(view) = guard.get_mut(&nm) else {
                    continue;
                };
                match ev {
                    SessionEvent::Connected => view.connected = true,
                    SessionEvent::Disconnected(reason) => {
                        tracing::info!(lamp = %nm, reason = %reason, "session ended");
                        view.connected = false;
                    }
                    SessionEvent::CommandFailed(reason) => {
                        tracing::warn!(lamp = %nm, reason = %reason, "command failed");
                        let msg = json!({ "type": "error", "lamp": nm, "error": reason });
                        let _ = tx2.send(msg.to_string());
                        continue;
                    }
                    SessionEvent::State(dp) => view.apply_dp(&dp),
                }
            }
            let snap = json!({ "type": "lamps", "lamps": *st.lock().unwrap() });
            let _ = tx2.send(snap.to_string());
        }
    });
}

/// SIGHUP → reload config: presets apply instantly (broadcast to clients),
/// lamp add/remove/key changes respawn the affected session tasks.
async fn reload_task(state: Shared) {
    let mut sighup = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "SIGHUP unavailable, hot reload disabled");
            return;
        }
    };
    while sighup.recv().await.is_some() {
        match crate::config::load(&state.cfg_path) {
            Ok(new) => apply_reload(&state, new).await,
            Err(e) => tracing::error!(
                error = %e,
                path = %state.cfg_path.display(),
                "reload failed, keeping old config"
            ),
        }
    }
}

async fn apply_reload(state: &Shared, new: Config) {
    let (mut added, mut removed, mut changed) = (0u32, 0u32, 0u32);
    {
        let mut cmds = state.cmds.lock().unwrap();
        let mut creds = state.creds.lock().unwrap();
        // Vanished or re-keyed lamps: dropping the sender ends their task.
        for name in creds.keys().cloned().collect::<Vec<_>>() {
            match new.lamps.get(&name) {
                None => {
                    cmds.remove(&name);
                    creds.remove(&name);
                    state.states.lock().unwrap().remove(&name);
                    removed += 1;
                }
                Some(lamp) if !same_creds(creds.get(&name).expect("creds has name"), lamp) => {
                    cmds.remove(&name);
                    let reg = LampRegistry {
                        idle: state.idle,
                        adapter: state.adapter.clone(),
                        tx: &state.tx,
                        states: &state.states,
                    };
                    start_lamp(&reg, &mut cmds, &mut creds, &name, lamp);
                    changed += 1;
                }
                Some(_) => {}
            }
        }
        for (name, lamp) in &new.lamps {
            if !creds.contains_key(name) {
                let reg = LampRegistry {
                    idle: state.idle,
                    adapter: state.adapter.clone(),
                    tx: &state.tx,
                    states: &state.states,
                };
                start_lamp(&reg, &mut cmds, &mut creds, name, lamp);
                added += 1;
            }
        }
    }
    *state.presets.lock().unwrap() = new.presets;
    tracing::info!(added, removed, changed, "config reloaded (SIGHUP)");
    let presets = state.presets.lock().unwrap().clone();
    let _ = state
        .tx
        .send(json!({ "type": "presets", "presets": presets }).to_string());
    let snap = json!({ "type": "lamps", "lamps": *state.states.lock().unwrap() });
    let _ = state.tx.send(snap.to_string());
}

/// True when a config lamp entry matches the running credentials.
fn same_creds(old: &LampCreds, lamp: &Lamp) -> bool {
    old.mac == lamp.mac
        && old.device_id == lamp.device_id
        && old.uuid == lamp.uuid
        && old.local_key == lamp.local_key
}

async fn index() -> impl IntoResponse {
    Html(APP_HTML)
}

async fn ws(State(state): State<Shared>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: Shared, mut socket: WebSocket) {
    tracing::debug!("ws client connected");
    let presets = state.presets.lock().unwrap().clone();
    let hello = json!({
        "type": "hello",
        "lamps": *state.states.lock().unwrap(),
        "presets": presets,
    });
    if socket
        .send(Message::Text(hello.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    let mut rx = state.tx.subscribe();
    loop {
        tokio::select! {
            res = rx.recv() => match res {
                Ok(msg) => {
                    if socket.send(Message::Text(msg.into())).await.is_err() {
                        tracing::debug!("ws: send failed, closing");
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = n, "ws client lagged behind broadcasts");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::debug!("ws: broadcast channel closed");
                    break;
                }
            },
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMsg>(&text) {
                            Ok(msg) => {
                                let lamp = msg.lamp().to_owned();
                                tracing::debug!(lamp = %lamp, raw = %text, "ws command");
                                if let Err(e) = apply(&state, msg).await {
                                    tracing::warn!(lamp = %lamp, error = %e, "client command failed");
                                    let err =
                                        json!({ "type": "error", "lamp": lamp, "error": e.to_string() });
                                    let _ = socket.send(Message::Text(err.to_string().into())).await;
                                }
                            }
                            Err(e) => {
                                let err = json!({ "type": "error", "error": format!("bad message: {e}") });
                                let _ = socket.send(Message::Text(err.to_string().into())).await;
                            }
                        }
                    }
                    Some(Ok(other)) => {
                        tracing::debug!(?other, "ws: non-text message ignored");
                    }
                    Some(Err(e)) => {
                        tracing::debug!(error = %e, "ws: recv error, closing");
                        break;
                    }
                    None => {
                        tracing::debug!("ws: socket closed by peer");
                        break;
                    }
                }
            }
        }
    }
    tracing::debug!("ws: handler exit");
}

async fn apply(state: &Shared, msg: ClientMsg) -> Result<()> {
    match msg {
        ClientMsg::Set { lamp, dp, value } => {
            let writes = build_set_writes(&dp, &value)?;
            send_cmd(state, &lamp, LampCmd::Write(writes)).await
        }
        ClientMsg::Preset { lamp, name } => {
            let p = state
                .presets
                .lock()
                .unwrap()
                .get(&name)
                .cloned()
                .with_context(|| format!("unknown preset {name}"))?;
            send_cmd(state, &lamp, LampCmd::Write(preset_writes(&p))).await
        }
        ClientMsg::Refresh { lamp } => send_cmd(state, &lamp, LampCmd::Status).await,
    }
}

async fn send_cmd(state: &Shared, lamp: &str, cmd: LampCmd) -> Result<()> {
    let tx = state
        .cmds
        .lock()
        .unwrap()
        .get(lamp)
        .cloned()
        .with_context(|| format!("unknown lamp {lamp}"))?;
    tx.send(cmd).await.context("lamp task stopped")
}

fn clamp_u32(v: &Value, lo: u32, hi: u32) -> Result<u32> {
    // i64 so out-of-range negatives clamp to lo instead of erroring as
    // "not a number" (as_u64 rejects them outright).
    let n = v.as_i64().context("expected a number")?;
    Ok(n.clamp(i64::from(lo), i64::from(hi)) as u32)
}

fn build_set_writes(dp: &str, value: &Value) -> Result<Vec<DpWrite>> {
    match dp {
        "on" => Ok(vec![DpWrite::bool(
            1,
            value.as_bool().context("on needs a bool")?,
        )]),
        "bright" => Ok(vec![DpWrite::value(3, clamp_u32(value, 10, 1000)?)]),
        "temp" => Ok(vec![DpWrite::value(4, clamp_u32(value, 0, 1000)?)]),
        "countdown" => Ok(vec![DpWrite::value(7, clamp_u32(value, 0, 86_400)?)]),
        "colour" => {
            let obj = value.as_object().context("colour needs {h,s,v}")?;
            let part = |k: &str| {
                clamp_u32(
                    obj.get(k).with_context(|| format!("colour needs {k}"))?,
                    0,
                    360,
                )
            };
            let h = part("h")?;
            let s = clamp_u32(obj.get("s").context("colour needs s")?, 0, 1000)?;
            let v = clamp_u32(obj.get("v").context("colour needs v")?, 0, 1000)?;
            // DP5 write auto-flips work_mode to colour (documented side-effect);
            // the explicit DP2 makes it deterministic.
            Ok(vec![DpWrite::colour(h, s, v), DpWrite::enum_val(2, 1)])
        }
        other => bail!("unknown dp {other}"),
    }
}

fn preset_writes(p: &Preset) -> Vec<DpWrite> {
    let mut out = Vec::new();
    if p.mode == "white" {
        out.push(DpWrite::enum_val(2, 0));
        if let Some(t) = p.temp {
            out.push(DpWrite::value(4, t));
        }
        if let Some(b) = p.brightness {
            out.push(DpWrite::value(3, b));
        }
    } else {
        // brightness FIRST: a DP3 write flips work_mode to white (scar — the
        // sleep preset must end on colour data, not brightness).
        if let Some(b) = p.brightness {
            out.push(DpWrite::value(3, b));
        }
        out.push(DpWrite::enum_val(2, 1));
        if let (Some(h), Some(s), Some(v)) = (p.hue, p.sat, p.val) {
            out.push(DpWrite::colour(h, s, v));
        }
    }
    out
}

/// Build the command for `--exec LAMP CMD [VALUE]` (CLI one-shot mode).
pub(crate) fn exec_cmd(cfg: &Config, parts: &[String]) -> Result<(String, LampCmd)> {
    let (lamp, cmd, value) = match parts {
        [lamp, cmd] => (lamp, cmd, None),
        [lamp, cmd, value] => (lamp, cmd, Some(value.as_str())),
        _ => bail!("--exec needs LAMP CMD [VALUE]"),
    };
    let num = |lo: u32, hi: u32| -> Result<u32> {
        let v: i64 = value
            .context("this command needs a value")?
            .parse()
            .context("bad number")?;
        Ok(v.clamp(i64::from(lo), i64::from(hi)) as u32)
    };
    let built = match cmd.as_str() {
        "on" | "off" => LampCmd::Write(vec![DpWrite::bool(1, cmd == "on")]),
        "bright" => LampCmd::Write(vec![DpWrite::value(3, num(10, 1000)?)]),
        "temp" => LampCmd::Write(vec![DpWrite::value(4, num(0, 1000)?)]),
        "countdown" => LampCmd::Write(vec![DpWrite::value(7, num(0, 86_400)?)]),
        "colour" => {
            let v = value.context("colour needs h,s,v")?;
            let parts: Vec<u32> = v
                .split(',')
                .map(|x| x.trim().parse().context("bad colour part"))
                .collect::<Result<Vec<_>, _>>()?;
            let [h, s, val] = parts.as_slice() else {
                bail!("colour needs h,s,v");
            };
            LampCmd::Write(vec![
                DpWrite::colour((*h).min(360), (*s).min(1000), (*val).min(1000)),
                DpWrite::enum_val(2, 1),
            ])
        }
        "preset" => {
            let name = value.context("preset needs a name")?;
            let p = cfg
                .presets
                .get(name)
                .with_context(|| format!("unknown preset {name}"))?;
            LampCmd::Write(preset_writes(p))
        }
        "status" => LampCmd::Status,
        other => {
            bail!("unknown command {other} (on|off|bright|temp|countdown|colour|preset|status)")
        }
    };
    Ok((lamp.clone(), built))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ids(writes: &[DpWrite]) -> Vec<u8> {
        writes.iter().map(|w| w.id).collect()
    }

    #[test]
    fn set_on_builds_bool_dp1() {
        let w = build_set_writes("on", &json!(true)).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].id, 1);
        assert_eq!(w[0].dtype, 1);
        assert_eq!(w[0].value, vec![1]);
    }

    #[test]
    fn set_on_rejects_non_bool() {
        assert!(build_set_writes("on", &json!("yes")).is_err());
        assert!(build_set_writes("on", &json!(1)).is_err());
    }

    #[test]
    fn set_bright_clamps_to_dp_range() {
        assert_eq!(
            build_set_writes("bright", &json!(5)).unwrap()[0].value,
            10u32.to_be_bytes()
        );
        assert_eq!(
            build_set_writes("bright", &json!(5000)).unwrap()[0].value,
            1000u32.to_be_bytes()
        );
        assert_eq!(
            build_set_writes("bright", &json!(500)).unwrap()[0].value,
            500u32.to_be_bytes()
        );
    }

    #[test]
    fn set_temp_targets_dp4() {
        let w = build_set_writes("temp", &json!(800)).unwrap();
        assert_eq!(w[0].id, 4);
        assert_eq!(w[0].dtype, 2);
    }

    #[test]
    fn set_colour_writes_data_then_mode() {
        let w = build_set_writes("colour", &json!({"h": 180, "s": 1000, "v": 1000})).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].id, 5);
        assert_eq!(w[0].dtype, 3);
        assert_eq!(w[0].value, b"00b403e803e8");
        assert_eq!(w[1].id, 2);
        assert_eq!(w[1].dtype, 4);
        assert_eq!(w[1].value, vec![1]);
    }

    #[test]
    fn set_colour_clamps_components() {
        let w = build_set_writes("colour", &json!({"h": 400, "s": 1200, "v": -5})).unwrap();
        assert_eq!(w[0].value, b"016803e80000");
    }

    #[test]
    fn set_colour_rejects_missing_parts() {
        assert!(build_set_writes("colour", &json!({"h": 1, "s": 1})).is_err());
        assert!(build_set_writes("colour", &json!("cyan")).is_err());
    }

    #[test]
    fn set_rejects_unknown_dp() {
        assert!(build_set_writes("scene", &json!(1)).is_err());
    }

    #[test]
    fn preset_white_orders_mode_temp_brightness() {
        let p = Preset {
            mode: "white".into(),
            temp: Some(800),
            brightness: Some(500),
            hue: None,
            sat: None,
            val: None,
        };
        assert_eq!(ids(&preset_writes(&p)), vec![2, 4, 3]);
    }

    #[test]
    fn preset_white_omits_missing_fields() {
        let p = Preset {
            mode: "white".into(),
            temp: None,
            brightness: None,
            hue: None,
            sat: None,
            val: None,
        };
        assert_eq!(ids(&preset_writes(&p)), vec![2]);
    }

    #[test]
    fn preset_colour_brightness_goes_first() {
        // Scar: a DP3 write flips work_mode to white, so with a colour preset
        // that sets brightness it MUST precede the mode + colour writes.
        let p = Preset {
            mode: "colour".into(),
            temp: None,
            brightness: Some(10),
            hue: Some(39),
            sat: Some(899),
            val: Some(104),
        };
        let w = preset_writes(&p);
        assert_eq!(ids(&w), vec![3, 2, 5]);
        assert_eq!(w[0].value, 10u32.to_be_bytes());
        assert_eq!(w[2].value, b"002703830068");
    }

    #[test]
    fn preset_colour_without_brightness() {
        let p = Preset {
            mode: "colour".into(),
            temp: None,
            brightness: None,
            hue: Some(180),
            sat: Some(1000),
            val: Some(1000),
        };
        assert_eq!(ids(&preset_writes(&p)), vec![2, 5]);
    }

    #[test]
    fn preset_colour_hue_defaults_cover_missing_hsv() {
        // hue/sat/val must be all-or-nothing: a partial preset writes no DP5
        // rather than a garbage colour.
        let p = Preset {
            mode: "colour".into(),
            temp: None,
            brightness: None,
            hue: Some(180),
            sat: None,
            val: None,
        };
        assert_eq!(ids(&preset_writes(&p)), vec![2]);
    }

    fn cfg_with_presets() -> Config {
        Config {
            lamps: BTreeMap::new(),
            presets: BTreeMap::from([
                (
                    "sleep".to_owned(),
                    Preset {
                        mode: "colour".into(),
                        temp: None,
                        brightness: Some(10),
                        hue: Some(39),
                        sat: Some(899),
                        val: Some(104),
                    },
                ),
                (
                    "warm-full".to_owned(),
                    Preset {
                        mode: "white".into(),
                        temp: Some(1000),
                        brightness: Some(1000),
                        hue: None,
                        sat: None,
                        val: None,
                    },
                ),
            ]),
        }
    }

    #[test]
    fn exec_on_off_and_clamped_bright() {
        let cfg = cfg_with_presets();
        let p = vec!["A".to_owned(), "on".to_owned()];
        let (lamp, cmd) = exec_cmd(&cfg, &p).unwrap();
        assert_eq!(lamp, "A");
        let LampCmd::Write(w) = cmd else { panic!() };
        assert_eq!(ids(&w), vec![1]);

        let p = ["A".to_owned(), "bright".to_owned(), "9999".to_owned()];
        let (_, cmd) = exec_cmd(&cfg, &p).unwrap();
        let LampCmd::Write(w) = cmd else { panic!() };
        assert_eq!(w[0].id, 3);
        assert_eq!(w[0].value, 1000u32.to_be_bytes());
    }

    #[test]
    fn exec_countdown_and_colour() {
        let cfg = cfg_with_presets();
        let p = ["A".to_owned(), "countdown".to_owned(), "900".to_owned()];
        let (_, cmd) = exec_cmd(&cfg, &p).unwrap();
        let LampCmd::Write(w) = cmd else { panic!() };
        assert_eq!(w[0].id, 7);
        assert_eq!(w[0].value, 900u32.to_be_bytes());

        let p = [
            "A".to_owned(),
            "colour".to_owned(),
            "400,1200,1000".to_owned(),
        ];
        let (_, cmd) = exec_cmd(&cfg, &p).unwrap();
        let LampCmd::Write(w) = cmd else { panic!() };
        assert_eq!(ids(&w), vec![5, 2]);
        // hue 400 clamps to 360 = 0x0168
        assert_eq!(w[0].value, b"016803e803e8");
    }

    #[test]
    fn exec_preset_and_status_and_errors() {
        let cfg = cfg_with_presets();
        let p = ["A".to_owned(), "preset".to_owned(), "sleep".to_owned()];
        let (_, cmd) = exec_cmd(&cfg, &p).unwrap();
        let LampCmd::Write(w) = cmd else { panic!() };
        // sleep has brightness: the DP3-first scar applies here too.
        assert_eq!(ids(&w), vec![3, 2, 5]);

        let p = ["A".to_owned(), "status".to_owned()];
        assert!(matches!(exec_cmd(&cfg, &p).unwrap().1, LampCmd::Status));

        let p = ["A".to_owned(), "preset".to_owned(), "nope".to_owned()];
        assert!(exec_cmd(&cfg, &p).is_err());
        let p = ["A".to_owned(), "frobnicate".to_owned()];
        assert!(exec_cmd(&cfg, &p).is_err());
    }
}
