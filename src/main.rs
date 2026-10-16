//! md rv-style entry point: clap CLI + logging init, then the app takes over.

mod app;
mod config;
mod tuya;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "mdrv-tuya")]
#[command(about = "BLE daemon + live UI for Tuya smart lamps (in-lite INSM044)")]
#[command(version)]
struct Args {
    /// Config file with lamp credentials and presets
    #[arg(short, long, default_value = "~/.config/mdrv-tuya/lamps.toml")]
    config: PathBuf,

    /// Bind address (use 0.0.0.0 to expose on the LAN)
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    hostname: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 4141)]
    port: u16,

    /// Disconnect a lamp after this many idle seconds (frees the bulb for the phone app)
    #[arg(short, long, default_value_t = 300)]
    idle_timeout: u64,

    /// Bluetooth adapter name (e.g. hci1); default = system default adapter
    #[arg(short, long)]
    adapter: Option<String>,

    /// Debug logging
    #[arg(short, long)]
    debug: bool,

    /// Trace logging (implies --debug)
    #[arg(long)]
    trace: bool,
}

fn init_logging(debug: bool, trace: bool) {
    let default_level = if trace {
        "trace"
    } else if debug {
        "debug"
    } else {
        "info"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

/// Expand a leading `~` using $HOME (avoiding a shellexpand dependency).
fn expand_home(p: &Path) -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        if let Ok(rel) = p.strip_prefix("~") {
            return PathBuf::from(home).join(rel);
        }
    }
    p.to_path_buf()
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_logging(args.debug, args.trace);
    // Panics inside spawned session tasks otherwise vanish; they must reach
    // the log because they can silently kill a WS connection or a lamp task.
    {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            tracing::error!(panic = %info, "panic");
            default_hook(info);
        }));
    }

    let cfg_path = expand_home(&args.config);
    let cfg = config::load(&cfg_path)
        .with_context(|| format!("loading config {}", cfg_path.display()))?;
    tracing::info!(
        path = %cfg_path.display(),
        lamps = cfg.lamps.len(),
        presets = cfg.presets.len(),
        "mdrv-tuya starting"
    );

    app::serve(
        cfg,
        &args.hostname,
        args.port,
        args.idle_timeout,
        args.adapter.clone(),
    )
    .await
    .context("daemon")
}
