//! axum HTTP + WebSocket surface: serves the embedded frontend and fans out
//! live lamp state to all connected UIs (kn26-style WS pattern).

use crate::config::{Config, Preset};
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
    }
}

struct AppState {
    presets: BTreeMap<String, Preset>,
    states: Arc<Mutex<BTreeMap<String, LampView>>>,
    tx: broadcast::Sender<String>,
    cmds: BTreeMap<String, mpsc::Sender<LampCmd>>,
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

pub async fn serve(
    cfg: Config,
    hostname: &str,
    port: u16,
    idle_timeout: u64,
    adapter: Option<String>,
) -> Result<()> {
    let idle = Duration::from_secs(idle_timeout);
    let (tx, _) = broadcast::channel::<String>(64);
    let states: Arc<Mutex<BTreeMap<String, LampView>>> = Arc::new(Mutex::new(
        cfg.lamps
            .keys()
            .map(|n| (n.clone(), LampView::new(n)))
            .collect(),
    ));

    let mut cmds = BTreeMap::new();
    for (name, lamp) in &cfg.lamps {
        let creds = LampCreds {
            name: name.clone(),
            mac: lamp.mac.clone(),
            device_id: lamp.device_id.clone(),
            uuid: lamp.uuid.clone(),
            local_key: lamp.local_key.clone(),
        };
        let (cmd_tx, cmd_rx) = mpsc::channel::<LampCmd>(32);
        let (ev_tx, mut ev_rx) = mpsc::channel::<SessionEvent>(64);
        cmds.insert(name.clone(), cmd_tx);

        tokio::spawn(crate::tuya::lamp_task(
            creds,
            idle,
            adapter.clone(),
            cmd_rx,
            ev_tx,
        ));

        let st = states.clone();
        let tx2 = tx.clone();
        let nm = name.clone();
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
                        SessionEvent::State(dp) => view.apply_dp(&dp),
                    }
                }
                let snap = json!({ "type": "lamps", "lamps": *st.lock().unwrap() });
                let _ = tx2.send(snap.to_string());
            }
        });
    }

    let state: Shared = Arc::new(AppState {
        presets: cfg.presets,
        states,
        tx,
        cmds,
    });

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

async fn index() -> impl IntoResponse {
    Html(APP_HTML)
}

async fn ws(State(state): State<Shared>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: Shared, mut socket: WebSocket) {
    tracing::debug!("ws client connected");
    let hello = json!({
        "type": "hello",
        "lamps": *state.states.lock().unwrap(),
        "presets": state.presets,
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
                                if let Err(e) = apply(&state, msg).await {
                                    tracing::warn!(error = %e, "client command failed");
                                    let err = json!({ "type": "error", "error": e.to_string() });
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
                .get(&name)
                .with_context(|| format!("unknown preset {name}"))?;
            send_cmd(state, &lamp, LampCmd::Write(preset_writes(p))).await
        }
        ClientMsg::Refresh { lamp } => send_cmd(state, &lamp, LampCmd::Status).await,
    }
}

async fn send_cmd(state: &Shared, lamp: &str, cmd: LampCmd) -> Result<()> {
    let tx = state
        .cmds
        .get(lamp)
        .with_context(|| format!("unknown lamp {lamp}"))?;
    tx.send(cmd).await.context("lamp task stopped")
}

fn clamp_u32(v: &Value, lo: u32, hi: u32) -> Result<u32> {
    let n = v.as_u64().context("expected a number")?;
    Ok(n.clamp(u64::from(lo), u64::from(hi)) as u32)
}

fn build_set_writes(dp: &str, value: &Value) -> Result<Vec<DpWrite>> {
    match dp {
        "on" => Ok(vec![DpWrite::bool(
            1,
            value.as_bool().context("on needs a bool")?,
        )]),
        "bright" => Ok(vec![DpWrite::value(3, clamp_u32(value, 10, 1000)?)]),
        "temp" => Ok(vec![DpWrite::value(4, clamp_u32(value, 0, 1000)?)]),
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
