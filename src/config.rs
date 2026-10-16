//! TOML config: lamp credentials + named presets (`~/.config/mdrv-tuya/lamps.toml`).
//!
//! Converted from the exploratory `/x/m/v270/tuya/lamps.jsonc`; that file stays
//! as documentation, this is the runtime source of truth.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Deserialize, Clone)]
pub struct Lamp {
    pub mac: String,
    pub device_id: String,
    pub uuid: String,
    pub local_key: String,
}

#[derive(Deserialize, Clone, serde::Serialize)]
pub struct Preset {
    /// "white" or "colour"
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sat: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub val: Option<u32>,
}

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(rename = "lamp")]
    pub lamps: BTreeMap<String, Lamp>,
    #[serde(rename = "preset")]
    pub presets: BTreeMap<String, Preset>,
}

pub fn load(path: &Path) -> Result<Config> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading config {}", path.display()))?;
    let cfg: Config =
        toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))?;
    Ok(cfg)
}
