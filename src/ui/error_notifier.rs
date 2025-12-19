use crate::app::{AppState, UiError};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_ERROR: u32 = WM_APP + 1;

pub const T_UI: &str = "UI";
pub const T_CONFIG: &str = "Config";
// pub const T_HOTKEYS: &str = "Hotkeys";
// pub const T_TRAY: &str = "Tray";
// pub const T_CONVERSION: &str = "Conversion";
// pub const T_STARTUP: &str = "Startup";

pub fn push(
    hwnd: HWND,
    state: &mut AppState,
    title: &str,
    user_text: &str,
    err: &windows::core::Error,
) {
    let debug_text = format!("{:?}", err);

    state.errors.push_back(UiError {
        title: title.to_string(),
        user_text: user_text.to_string(),
        debug_text,
    });

    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_APP_ERROR, WPARAM(0), LPARAM(0));
    }
}

pub fn drain_one(state: &mut AppState) -> Option<UiError> {
    state.errors.pop_front()
}

pub fn report_unit(
    hwnd: HWND,
    state: &mut AppState,
    title: &str,
    user_text: &str,
    r: windows::core::Result<()>,
) {
    if let Err(e) = r {
        push(hwnd, state, title, user_text, &e);
    }
}

#[macro_export]
macro_rules! ui_try {
    ($hwnd:expr, $state:expr, $title:expr, $text:expr, $expr:expr) => {{
        $crate::ui::error_notifier::report_unit($hwnd, $state, $title, $text, $expr);
    }};
}

#[macro_export]
macro_rules! ui_opt {
    ($hwnd:expr, $state:expr, $title:expr, $text:expr, $expr:expr) => {{ $crate::ui::error_notifier::report($hwnd, $state, $title, $text, $expr) }};
}

#[macro_export]
macro_rules! ui_call {
    ($hwnd:expr, $state:expr, $title:expr, $text:expr, $call:expr) => {{
        let r = $call;
        $crate::ui::error_notifier::report_unit($hwnd, $state, $title, $text, r);
    }};
}
