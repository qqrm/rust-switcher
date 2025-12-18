//! Defines the application state and associated constants.
//!
//! The `AppState` structure holds handles to all of the controls in
//! the settings window along with the font used for drawing text.
//! Constants representing control identifiers are defined here so
//! that they can be shared between modules.

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::HFONT;
use windows::Win32::UI::WindowsAndMessaging::HMENU;

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
}

#[derive(Debug, Default)]
pub struct Checkboxes {
    pub autostart: HWND,
    pub tray: HWND,
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

/// Control identifiers used in WM_COMMAND and as HMENU in CreateWindowExW.
#[repr(i32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ControlId {
    Autostart = 1001,
    Tray = 1002,
    DelayMs = 1003,

    Apply = 1101,
    Cancel = 1102,
    Exit = 1103,
}

impl ControlId {
    #[inline]
    pub const fn hmenu(self) -> Option<HMENU> {
        Some(HMENU(self as i32 as usize as *mut c_void))
    }
}