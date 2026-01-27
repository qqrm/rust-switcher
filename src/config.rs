mod config_validator;
pub mod constants;
pub mod raw_config;

use std::{
    io,
    path::{Path, PathBuf},
};

pub use constants::{
    MODVK_LALT, MODVK_LCTRL, MODVK_LSHIFT, MODVK_LWIN, MODVK_RALT, MODVK_RCTRL, MODVK_RSHIFT,
    MODVK_RWIN,
};
pub use raw_config::RawConfig;
use serde::{Deserialize, Deserializer, Serialize};

const APP_DIR: &str = "RustSwitcher";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Hotkey {
    pub vk: u32,
    pub mods: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeyChord {
    pub mods: u32,

    #[serde(default)]
    pub mods_vks: u32,

    pub vk: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeySequence {
    pub first: HotkeyChord,
    pub second: Option<HotkeyChord>,
    pub max_gap_ms: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Config {
    pub delay_ms: u32,
    #[serde(default)]
    start_minimized: bool,
    #[serde(default)]
    theme_dark: bool,

    hotkey_convert_last_word: Option<Hotkey>,
    hotkey_convert_selection: Option<Hotkey>,
    hotkey_switch_layout: Option<Hotkey>,
    hotkey_pause: Option<Hotkey>,

    #[serde(default)]
    hotkey_convert_last_word_sequence: Option<HotkeySequence>,
    #[serde(default)]
    hotkey_pause_sequence: Option<HotkeySequence>,
    #[serde(default)]
    hotkey_convert_selection_sequence: Option<HotkeySequence>,
    #[serde(default)]
    hotkey_switch_layout_sequence: Option<HotkeySequence>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            delay_ms: 100,
            start_minimized: false,
            theme_dark: false,

            hotkey_switch_layout: None,
            hotkey_pause: None,
            hotkey_convert_last_word: None,
            hotkey_convert_selection: None,

            hotkey_convert_last_word_sequence: Some(HotkeySequence {
                first: HotkeyChord {
                    mods: 4,
                    mods_vks: 4,
                    vk: None,
                },
                second: Some(HotkeyChord {
                    mods: 4,
                    mods_vks: 4,
                    vk: None,
                }),
                max_gap_ms: 1000,
            }),

            hotkey_pause_sequence: Some(HotkeySequence {
                first: HotkeyChord {
                    mods: 4,
                    mods_vks: 12,
                    vk: None,
                },
                second: None,
                max_gap_ms: 1000,
            }),

            hotkey_convert_selection_sequence: Some(HotkeySequence {
                first: HotkeyChord {
                    mods: 4,
                    mods_vks: 4,
                    vk: None,
                },
                second: Some(HotkeyChord {
                    mods: 4,
                    mods_vks: 4,
                    vk: None,
                }),
                max_gap_ms: 1000,
            }),

            hotkey_switch_layout_sequence: Some(HotkeySequence {
                first: HotkeyChord {
                    mods: 0,
                    mods_vks: 0,
                    vk: Some(20),
                },
                second: None,
                max_gap_ms: 1000,
            }),
        }
    }
}

pub fn config_path() -> io::Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA is not set"))?;

    Ok(PathBuf::from(appdata).join(APP_DIR).join(CONFIG_FILE))
}

fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir)
}

fn confy_err(e: confy::ConfyError) -> io::Error {
    io::Error::other(e)
}

pub fn load() -> io::Result<Config> {
    let path = config_path()?;
    ensure_parent_dir(&path)?;

    confy::load_path(&path).map_err(confy_err)
}

#[allow(dead_code)]
pub fn save(cfg: &Config) -> io::Result<()> {
    let path = config_path()?;
    ensure_parent_dir(&path)?;
    confy::store_path(path, cfg).map_err(confy_err)
}

impl TryFrom<RawConfig> for Config {
    type Error = String;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        raw.validate_hotkey_sequences()?;

        Ok(Self {
            delay_ms: raw.delay_ms,
            start_minimized: raw.start_minimized,
            theme_dark: raw.theme_dark,
            hotkey_convert_last_word: raw.hotkey_convert_last_word,
            hotkey_convert_selection: raw.hotkey_convert_selection,
            hotkey_switch_layout: raw.hotkey_switch_layout,
            hotkey_pause: raw.hotkey_pause,
            hotkey_convert_last_word_sequence: raw.hotkey_convert_last_word_sequence,
            hotkey_pause_sequence: raw.hotkey_pause_sequence,
            hotkey_convert_selection_sequence: raw.hotkey_convert_selection_sequence,
            hotkey_switch_layout_sequence: raw.hotkey_switch_layout_sequence,
        })
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawConfig::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl Config {
    pub fn hotkey_convert_last_word(&self) -> Option<Hotkey> {
        self.hotkey_convert_last_word
    }

    pub fn hotkey_convert_selection(&self) -> Option<Hotkey> {
        self.hotkey_convert_selection
    }

    pub fn hotkey_switch_layout(&self) -> Option<Hotkey> {
        self.hotkey_switch_layout
    }

    pub fn hotkey_pause(&self) -> Option<Hotkey> {
        self.hotkey_pause
    }

    pub fn hotkey_convert_last_word_sequence(&self) -> Option<HotkeySequence> {
        self.hotkey_convert_last_word_sequence
    }

    pub fn hotkey_pause_sequence(&self) -> Option<HotkeySequence> {
        self.hotkey_pause_sequence
    }

    pub fn hotkey_convert_selection_sequence(&self) -> Option<HotkeySequence> {
        self.hotkey_convert_selection_sequence
    }

    pub fn hotkey_switch_layout_sequence(&self) -> Option<HotkeySequence> {
        self.hotkey_switch_layout_sequence
    }

    pub fn start_minimized(&self) -> bool {
        self.start_minimized
    }

    pub fn theme_dark(&self) -> bool {
        self.theme_dark
    }

    pub fn set_theme_dark(&mut self, value: bool) {
        self.theme_dark = value;
    }
}
