use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_APP},
};

use crate::app::{AppState, UiError};

/// Application private message used to signal that the UI error queue is non empty.
///
/// The window procedure is expected to handle this message and drain one or more
/// items from `AppState::errors`.
pub const WM_APP_ERROR: u32 = WM_APP + 1;

/// Standard error title tag for UI subsystem messages.
pub const T_UI: &str = "UI";

/// Standard error title tag for configuration related messages.
pub const T_CONFIG: &str = "Config";

/// Enqueues a UI error into `state.errors` and schedules UI processing via `WM_APP_ERROR`.
///
/// This function does not show any UI itself. It only:
/// 1) stores an error payload in the state queue
/// 2) posts `WM_APP_ERROR` to the target window, letting the window procedure handle display
///
/// Rationale:
/// Posting a message decouples error production from error presentation and keeps all
/// UI operations inside the window message loop.
///
/// Safety:
/// This function calls Win32 `PostMessageW`. The `hwnd` must be a valid window handle.
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
        _debug_text: debug_text,
    });

    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_APP_ERROR, WPARAM(0), LPARAM(0));
    }
}

/// Pops a single error from the UI error queue.
///
/// The intended consumer is the window procedure that receives `WM_APP_ERROR`.
pub fn drain_one(state: &mut AppState) -> Option<UiError> {
    state.errors.pop_front()
}

/// Reports a `Result<()>` by enqueuing an error on `Err`.
///
/// This is a convenience helper for UI facing operations where failures should be
/// surfaced through the notifier pipeline.
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

/// Evaluates an expression that returns `Result<()>` and routes an error into the UI notifier.
///
/// Usage pattern:
/// `ui_try!(hwnd, state, T_UI, "message", some_call());`
#[macro_export]
macro_rules! ui_try {
    ($hwnd:expr, $state:expr, $title:expr, $text:expr, $expr:expr) => {{
        $crate::ui::error_notifier::report_unit($hwnd, $state, $title, $text, $expr);
    }};
}

/// Calls the provided closure or expression, stores the result in a temporary, then reports `Err`.
///
/// This is useful when the call is not a simple expression and you want a named `r` for debugging.
#[macro_export]
macro_rules! ui_call {
    ($hwnd:expr, $state:expr, $title:expr, $text:expr, $call:expr) => {{
        let r = $call;
        $crate::ui::error_notifier::report_unit($hwnd, $state, $title, $text, r);
    }};
}
