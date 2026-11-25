use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::audio::Track;
use crate::visualizer::VisMode;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct PersistedState {
    pub volume: f32,
    pub balance: f32,
    pub shuffle: bool,
    pub repeat: bool,
    pub show_eq: bool,
    pub show_playlist: bool,
    pub windowshade: bool,
    pub remaining_time_mode: bool,
    pub vis_mode: VisMode,
    pub eq_enabled: bool,
    pub eq_preamp_db: f32,
    pub eq_bands_db: [f32; 10],
    pub eq_preset_name: String,
    pub playlist: Vec<Track>,
    pub current_index: Option<usize>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            volume: 0.8,
            balance: 0.0,
            shuffle: false,
            repeat: false,
            show_eq: true,
            show_playlist: true,
            windowshade: false,
            remaining_time_mode: false,
            vis_mode: VisMode::Spectrum,
            eq_enabled: true,
            eq_preamp_db: 0.0,
            eq_bands_db: [0.0; 10],
            eq_preset_name: "Flat".to_string(),
            playlist: Vec::new(),
            current_index: None,
        }
    }
}

pub fn state_file_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("RustAmp").join("state.json")
    } else {
        PathBuf::from("rustamp_state.json")
    }
}

pub fn load_state() -> Option<PersistedState> {
    let path = state_file_path();
    let contents = fs::read_to_string(&path).ok().or_else(|| {
        std::env::var("APPDATA")
            .ok()
            .and_then(|a| fs::read_to_string(PathBuf::from(a).join("WinampR").join("state.json")).ok())
    })?;
    serde_json::from_str(&contents).ok()
}

pub fn save_state(state: &PersistedState) {
    let path = state_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(&path, json);
    }
}
