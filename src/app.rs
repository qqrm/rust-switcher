//! Defines the application state and associated constants.
//!
//! The `AppState` structure holds handles to all of the controls in
//! the settings window along with the font used for drawing text.
//! Constants representing control identifiers are defined here so
//! that they can be shared between modules.

use std::collections::VecDeque;

use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{HBRUSH, HFONT},
    UI::WindowsAndMessaging::HMENU,
};

use crate::config;

#[derive(Debug, Clone)]
pub struct UiError {
    pub title: String,
    pub user_text: String,
    pub _debug_text: String,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum HotkeySlot {
    LastWord,
    Pause,
    Selection,
    SwitchLayout,
}

#[derive(Debug, Default, Clone)]
pub struct HotkeyValues {
    pub last_word: Option<config::Hotkey>,
    pub pause: Option<config::Hotkey>,
    pub selection: Option<config::Hotkey>,
    pub switch_layout: Option<config::Hotkey>,
}

impl HotkeyValues {
    pub fn from_config(cfg: &config::Config) -> Self {
        Self {
            last_word: cfg.hotkey_convert_last_word(),
            pause: cfg.hotkey_pause(),
            selection: cfg.hotkey_convert_selection(),
            switch_layout: cfg.hotkey_switch_layout(),
        }
    }

    #[allow(dead_code)]
    pub fn get(&self, slot: HotkeySlot) -> Option<config::Hotkey> {
        match slot {
            HotkeySlot::LastWord => self.last_word,
            HotkeySlot::Pause => self.pause,
            HotkeySlot::Selection => self.selection,
            HotkeySlot::SwitchLayout => self.switch_layout,
        }
    }

    pub fn set(&mut self, slot: HotkeySlot, hk: Option<config::Hotkey>) {
        match slot {
            HotkeySlot::LastWord => self.last_word = hk,
            HotkeySlot::Pause => self.pause = hk,
            HotkeySlot::Selection => self.selection = hk,
            HotkeySlot::SwitchLayout => self.switch_layout = hk,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct HotkeySequenceValues {
    pub last_word: Option<config::HotkeySequence>,
    pub pause: Option<config::HotkeySequence>,
    pub selection: Option<config::HotkeySequence>,
    pub switch_layout: Option<config::HotkeySequence>,
}

impl HotkeySequenceValues {
    pub fn from_config(cfg: &config::Config) -> Self {
        Self {
            last_word: cfg.hotkey_convert_last_word_sequence(),
            pause: cfg.hotkey_pause_sequence(),
            selection: cfg.hotkey_convert_selection_sequence(),
            switch_layout: cfg.hotkey_switch_layout_sequence(),
        }
    }

    pub fn get(&self, slot: HotkeySlot) -> Option<config::HotkeySequence> {
        match slot {
            HotkeySlot::LastWord => self.last_word,
            HotkeySlot::Pause => self.pause,
            HotkeySlot::Selection => self.selection,
            HotkeySlot::SwitchLayout => self.switch_layout,
        }
    }

    pub fn set(&mut self, slot: HotkeySlot, seq: Option<config::HotkeySequence>) {
        match slot {
            HotkeySlot::LastWord => self.last_word = seq,
            HotkeySlot::Pause => self.pause = seq,
            HotkeySlot::Selection => self.selection = seq,
            HotkeySlot::SwitchLayout => self.switch_layout = seq,
        }
    }
}

#[derive(Debug, Default)]
pub struct HotkeyCaptureUi {
    pub active: bool,
    pub slot: Option<HotkeySlot>,

    pub pending_mods_vks: u32,
    pub pending_mods: u32,
    pub pending_mods_valid: bool,
    pub saw_non_mod: bool,

    // Last successful chord capture time in milliseconds since boot (GetTickCount64).
    // Used to reset sequence after a long pause.
    pub last_input_tick_ms: u64,
}

#[derive(Debug, Default)]
pub struct RuntimeChordCapture {
    pub pending_mods_vks: u32,
    pub pending_mods: u32,
    pub pending_mods_valid: bool,
    pub saw_non_mod: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SequenceProgress {
    pub waiting_second: bool,
    pub first_tick_ms: u64,
}

#[derive(Debug, Default)]
pub struct HotkeySequenceProgress {
    pub last_word: SequenceProgress,
    pub pause: SequenceProgress,
    pub selection: SequenceProgress,
    pub switch_layout: SequenceProgress,
}

/// Per-window state used throughout the application.
///
/// Stored in window user data. Contains handles of child controls and UI resources.
#[derive(Debug, Default)]
pub struct AppState {
    /// Font used for all controls. Assigned after window creation.
    pub font: HFONT,

    pub checkboxes: Checkboxes,
    pub edits: Edits,
    pub hotkeys: HotkeyEdits,
    pub buttons: Buttons,

    pub autoconvert_enabled: bool,
    pub errors: VecDeque<UiError>,

    /// Temporary hotkeys currently shown in UI. Committed on Apply.
    pub hotkey_values: HotkeyValues,
    pub hotkey_sequence_values: HotkeySequenceValues,

    /// Which hotkey edit is currently capturing input.
    pub hotkey_capture: HotkeyCaptureUi,

    /// Active (already applied) sequences used by the runtime hotkey recognizer.
    /// This must NOT be tied to temporary edits in the UI.
    pub active_hotkey_sequences: HotkeySequenceValues,

    /// Runtime state for modifier-only chord detection.
    pub runtime_chord_capture: RuntimeChordCapture,

    /// Runtime state for chord sequence progress.
    pub hotkey_sequence_progress: HotkeySequenceProgress,

    pub active_switch_layout_sequence: Option<config::HotkeySequence>,
    pub switch_layout_waiting_second: bool,
    pub switch_layout_first_tick_ms: u64,

    pub current_theme_dark: bool,
    // Cached theme brushes (must be deleted on window destroy)
    pub dark_brush_window_bg: HBRUSH,
    pub dark_brush_control_bg: HBRUSH,
    pub dark_brush_edit_bg: HBRUSH,
}

#[derive(Debug, Default)]
pub struct Checkboxes {
    pub autostart: HWND,
    pub start_minimized: HWND,
    pub theme_dark: HWND,
}

#[derive(Debug, Default)]
pub struct Edits {
    pub delay_ms: HWND,
}

#[derive(Debug, Default)]
pub struct HotkeyEdits {
    pub last_word: HWND,
    pub pause: HWND,
    pub selection: HWND,
    pub switch_layout: HWND,
}

#[derive(Debug, Default)]
pub struct Buttons {
    pub apply: HWND,
    pub cancel: HWND,
    pub exit: HWND,
}

/// Control identifiers used in `WM_COMMAND` and as `HMENU` in `CreateWindowExW`.
#[repr(i32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ControlId {
    Autostart = 1001,
    Tray = 1002,
    DelayMs = 1003,
    StartMinimized = 1004,
    DarkTheme = 1005,

    HotkeyLastWord = 1201,
    HotkeyPause = 1202,
    HotkeySelection = 1203,
    HotkeySwitchLayout = 1204,

    Apply = 1101,
    Cancel = 1102,
    Exit = 1103,
}

impl ControlId {
    #[inline]
    pub const fn from_i32(v: i32) -> Option<Self> {
        match v {
            1001 => Some(Self::Autostart),
            1002 => Some(Self::Tray),
            1003 => Some(Self::DelayMs),
            1004 => Some(Self::StartMinimized),
            1005 => Some(Self::DarkTheme),

            1201 => Some(Self::HotkeyLastWord),
            1202 => Some(Self::HotkeyPause),
            1203 => Some(Self::HotkeySelection),
            1204 => Some(Self::HotkeySwitchLayout),

            1101 => Some(Self::Apply),
            1102 => Some(Self::Cancel),
            1103 => Some(Self::Exit),

            _ => None,
        }
    }

    #[inline]
    pub fn hmenu(self) -> windows::Win32::UI::WindowsAndMessaging::HMENU {
        use std::ffi::c_void;

        let id = self as u16;
        HMENU(usize::from(id) as *mut c_void)
    }
}
