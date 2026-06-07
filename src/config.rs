mod config_validator;
pub mod constants;

use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT};

const CONFIG_FILE: &str = "config.json";
pub const CONVERSION_DELAY_MS: u32 = 100;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Hotkey {
    pub vk: u32,
    pub mods: u32,
}

pub const MODVK_LCTRL: u32 = 1 << 0;
pub const MODVK_RCTRL: u32 = 1 << 1;
pub const MODVK_LSHIFT: u32 = 1 << 2;
pub const MODVK_RSHIFT: u32 = 1 << 3;
pub const MODVK_LALT: u32 = 1 << 4;
pub const MODVK_RALT: u32 = 1 << 5;
pub const MODVK_LWIN: u32 = 1 << 6;
pub const MODVK_RWIN: u32 = 1 << 7;

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
    #[serde(default)]
    pub third: Option<HotkeyChord>,
    pub max_gap_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub delay_ms: u32,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub theme_dark: bool,

    pub hotkey_convert_last_word: Option<Hotkey>,
    #[serde(default)]
    pub hotkey_convert_last_sequence: Option<Hotkey>,
    pub hotkey_convert_selection: Option<Hotkey>,
    pub hotkey_switch_layout: Option<Hotkey>,
    pub hotkey_pause: Option<Hotkey>,

    #[serde(default)]
    pub hotkey_convert_last_word_sequence: Option<HotkeySequence>,
    #[serde(default = "default_hotkey_convert_last_sequence_sequence")]
    pub hotkey_convert_last_sequence_sequence: Option<HotkeySequence>,
    #[serde(default)]
    pub hotkey_pause_sequence: Option<HotkeySequence>,
    #[serde(default)]
    pub hotkey_convert_selection_sequence: Option<HotkeySequence>,
    #[serde(default)]
    pub hotkey_switch_layout_sequence: Option<HotkeySequence>,

    #[serde(default)]
    pub smarter_hotkeys_enabled: bool,
    #[serde(default = "default_smart_hotkey_convert_last_word_sequence")]
    pub smart_hotkey_convert_last_word_sequence: Option<HotkeySequence>,
    #[serde(default = "default_smart_hotkey_convert_last_sequence_sequence")]
    pub smart_hotkey_convert_last_sequence_sequence: Option<HotkeySequence>,
    #[serde(default = "default_smart_hotkey_convert_selection_sequence")]
    pub smart_hotkey_convert_selection_sequence: Option<HotkeySequence>,
}

fn modifier_only_chord(mods: u32, mods_vks: u32) -> HotkeyChord {
    HotkeyChord {
        mods,
        mods_vks,
        vk: None,
    }
}

fn double_modifier_sequence(mods: u32, mods_vks: u32) -> HotkeySequence {
    HotkeySequence {
        first: modifier_only_chord(mods, mods_vks),
        second: Some(modifier_only_chord(mods, mods_vks)),
        third: None,
        max_gap_ms: 1000,
    }
}

fn triple_modifier_sequence(mods: u32, mods_vks: u32) -> HotkeySequence {
    HotkeySequence {
        first: modifier_only_chord(mods, mods_vks),
        second: Some(modifier_only_chord(mods, mods_vks)),
        third: Some(modifier_only_chord(mods, mods_vks)),
        max_gap_ms: 1000,
    }
}

fn default_hotkey_convert_last_sequence_sequence() -> Option<HotkeySequence> {
    Some(double_modifier_sequence(MOD_ALT.0, MODVK_LALT))
}

fn default_smart_hotkey_convert_last_word_sequence() -> Option<HotkeySequence> {
    Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT))
}

fn default_smart_hotkey_convert_last_sequence_sequence() -> Option<HotkeySequence> {
    Some(triple_modifier_sequence(MOD_ALT.0, MODVK_LALT))
}

fn default_smart_hotkey_convert_selection_sequence() -> Option<HotkeySequence> {
    Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT))
}

impl Default for Config {
    fn default() -> Self {
        Self {
            delay_ms: CONVERSION_DELAY_MS,
            start_minimized: false,
            theme_dark: false,

            hotkey_switch_layout: None,
            hotkey_pause: None,
            hotkey_convert_last_word: None,
            hotkey_convert_last_sequence: None,
            hotkey_convert_selection: None,

            hotkey_convert_last_word_sequence: Some(double_modifier_sequence(
                MOD_SHIFT.0,
                MODVK_LSHIFT,
            )),
            hotkey_convert_last_sequence_sequence: default_hotkey_convert_last_sequence_sequence(),

            hotkey_pause_sequence: Some(double_modifier_sequence(MOD_CONTROL.0, MODVK_RCTRL)),
            hotkey_convert_selection_sequence: Some(double_modifier_sequence(
                MOD_SHIFT.0,
                MODVK_LSHIFT,
            )),
            hotkey_switch_layout_sequence: Some(HotkeySequence {
                first: HotkeyChord {
                    mods: 0,
                    mods_vks: 0,
                    vk: Some(20),
                },
                second: None,
                third: None,
                max_gap_ms: 1000,
            }),

            smarter_hotkeys_enabled: false,
            smart_hotkey_convert_last_word_sequence:
                default_smart_hotkey_convert_last_word_sequence(),
            smart_hotkey_convert_last_sequence_sequence:
                default_smart_hotkey_convert_last_sequence_sequence(),
            smart_hotkey_convert_selection_sequence:
                default_smart_hotkey_convert_selection_sequence(),
        }
    }
}

pub fn config_path() -> io::Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA is not set"))?;

    Ok(PathBuf::from(appdata)
        .join(crate::app_identity::APP_DIR)
        .join(CONFIG_FILE))
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
    confy::load_path(path).map_err(confy_err)
}

pub fn save(cfg: &Config) -> io::Result<()> {
    let path = config_path()?;
    ensure_parent_dir(&path)?;
    cfg.validate_hotkey_sequences()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    confy::store_path(path, cfg).map_err(confy_err)
}
