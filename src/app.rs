//! Defines the application state and associated constants.
//!
//! The `AppState` structure holds handles to all of the controls in
//! the settings window along with the font used for drawing text.
//! Constants representing control identifiers are defined here so
//! that they can be shared between modules.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::HFONT;

/// Per‑window state used throughout the application.
///
/// An instance of `AppState` is allocated when the main window is
/// created and stored in the window user data.  Each control
/// created in the UI stores its window handle in one of the fields
/// of this struct so that event handlers can easily access them.
#[derive(Default)]
pub struct AppState {
    /// Font used for all controls.  Assigned after the window is
    /// created via `visuals::create_message_font`.
    pub font: HFONT,

    pub chk_autostart: HWND,
    pub chk_tray: HWND,
    pub edit_delay_ms: HWND,

    pub edit_hotkey_last_word: HWND,
    pub edit_hotkey_pause: HWND,
    pub edit_hotkey_selection: HWND,
    pub edit_hotkey_switch_layout: HWND,

    pub btn_apply: HWND,
    pub btn_cancel: HWND,
    pub btn_exit: HWND,
}

// Control identifiers.  These values are passed as the `HMENU`
// parameter to `CreateWindowExW` so they are available in
// `WM_COMMAND` notifications.

/// Identifier for the "Start on startup" checkbox.
pub const ID_AUTOSTART: i32 = 1001;
/// Identifier for the "Show tray icon" checkbox.
pub const ID_TRAY: i32 = 1002;
/// Identifier for the delay edit box.
pub const ID_DELAY_MS: i32 = 1003;

/// Identifier for the Apply button.
pub const ID_APPLY: i32 = 1101;
/// Identifier for the Cancel button.
pub const ID_CANCEL: i32 = 1102;
/// Identifier for the Exit button.
pub const ID_EXIT: i32 = 1103;