//! TOML config: lamp credentials + named presets (`~/.config/mdrv-tuya/mdrv-tuya.toml`).
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[lamp.A]
mac = "DC:23:4D:99:67:A4"
device_id = "a3a730ntk1amjg7w"
uuid = "8bdbbe48f5f4e020"
local_key = "oZ$^J|QnRg}ZZYP3"

[preset.sleep]
mode = "colour"
hue = 39
sat = 899
val = 104
brightness = 10

[preset.warm-full]
mode = "white"
temp = 1000
brightness = 1000
"#;

    #[test]
    fn parses_lamps_and_presets() {
        let cfg: Config = toml::from_str(SAMPLE).unwrap();
        let a = cfg.lamps.get("A").unwrap();
        assert_eq!(a.mac, "DC:23:4D:99:67:A4");
        assert_eq!(a.local_key, "oZ$^J|QnRg}ZZYP3");
        let sleep = cfg.presets.get("sleep").unwrap();
        assert_eq!(sleep.mode, "colour");
        assert_eq!(sleep.brightness, Some(10));
        assert_eq!(sleep.hue, Some(39));
        assert!(sleep.temp.is_none());
        assert_eq!(cfg.presets.get("warm-full").unwrap().temp, Some(1000));
    }

    #[test]
    fn preset_serialization_skips_missing_fields() {
        let p = Preset {
            mode: "white".into(),
            temp: Some(500),
            brightness: None,
            hue: None,
            sat: None,
            val: None,
        };
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(s, r#"{"mode":"white","temp":500}"#);
    }

    #[test]
    fn missing_preset_fields_default_to_none() {
        let cfg: Config = toml::from_str(SAMPLE).unwrap();
        let warm = cfg.presets.get("warm-full").unwrap();
        assert_eq!(warm.hue, None);
        assert_eq!(warm.val, None);
    }
}
