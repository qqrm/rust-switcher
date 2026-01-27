use serde::Deserialize;

use crate::config::*;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RawConfig {
    pub delay_ms: u32,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub theme_dark: bool,

    pub hotkey_convert_last_word: Option<Hotkey>,
    pub hotkey_convert_selection: Option<Hotkey>,
    pub hotkey_switch_layout: Option<Hotkey>,
    pub hotkey_pause: Option<Hotkey>,

    #[serde(default)]
    pub hotkey_convert_last_word_sequence: Option<HotkeySequence>,
    #[serde(default)]
    pub hotkey_pause_sequence: Option<HotkeySequence>,
    #[serde(default)]
    pub hotkey_convert_selection_sequence: Option<HotkeySequence>,
    #[serde(default)]
    pub hotkey_switch_layout_sequence: Option<HotkeySequence>,
}
