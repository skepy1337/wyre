// .config/wyre/settings.json
use crate::geoip::GeoMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    pub geo_mode: Option<GeoMode>,
    #[serde(default)]
    pub imported: Vec<PathBuf>,
    #[serde(default)]
    pub view: ViewSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewSettings {
    pub show_tcp: bool,
    pub show_udp: bool,
    pub show_icmp: bool,
    pub hide_dns: bool,
    pub zoom: f64,
}

impl Default for ViewSettings {
    fn default() -> Self {
        ViewSettings {
            show_tcp: true,
            show_udp: true,
            show_icmp: true,
            hide_dns: false,
            zoom: 1.0,
        }
    }
}

const ZOOM_LEVELS: [f64; 11] =
    [0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

pub fn step_zoom(zoom: f64, zoom_in: bool) -> f64 {
    let next = if zoom_in {
        ZOOM_LEVELS.iter().find(|&&level| level > zoom)
    } else {
        ZOOM_LEVELS.iter().rev().find(|&&level| level < zoom)
    };
    next.copied().unwrap_or(zoom)
}

pub fn load() -> Settings {
    let json = fs::read_to_string(path()).unwrap_or_default();
    let mut settings: Settings = serde_json::from_str(&json).unwrap_or_default();

    let zoom = settings.view.zoom;
    let (min, max) = (ZOOM_LEVELS[0], ZOOM_LEVELS[ZOOM_LEVELS.len() - 1]);
    settings.view.zoom = if zoom.is_finite() { zoom.clamp(min, max) } else { 1.0 };
    settings
}

pub fn save(settings: &Settings) {
    let path = path();
    let json = serde_json::to_string_pretty(settings).expect("settings serialize");
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Err(e) = fs::write(&path, json) {
        eprintln!("Couldn't save {}: {e}", path.display());
    }
}

fn path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
    config_dir.join("wyre/settings.json")
}
