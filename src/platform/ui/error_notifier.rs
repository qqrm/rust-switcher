use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_APP},
};

use crate::{
    app::{AppState, UiError},
    platform::win::tray::balloon_error,
};

/// Application private message used to signal that the UI error queue is non empty.
///
/// The window procedure is expected to handle this message and drain one or more
/// items from `AppState::errors`.
pub const WM_APP_ERROR: u32 = WM_APP + 1;
pub const WM_APP_AUTOCONVERT: u32 = WM_APP + 2;

/// Standard error title tag for UI subsystem messages.
pub const T_UI: &str = "UI";

/// Standard error title tag for configuration related messages.
pub const T_CONFIG: &str = "Config";

/// Drains one queued error and presents it to the user.
///
/// Presentation strategy:
/// - primary: tray balloon notification
/// - fallback: `MessageBoxW` if the tray balloon fails
///
/// This function must be called on the UI thread.
pub fn drain_one_and_present(hwnd: HWND, state: &mut AppState) {
    use windows::{
        Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
        core::HSTRING,
    };

    let Some(err) = drain_one(state) else {
        return;
    };

    let show_message_box = |title: &str, text: &str| unsafe {
        let _ = MessageBoxW(
            Some(hwnd),
            &HSTRING::from(text),
            &HSTRING::from(title),
            MB_OK | MB_ICONERROR,
        );
    };

    balloon_error(hwnd, &err.title, &err.user_text)
        .inspect_err(|_| show_message_box(&err.title, &err.user_text))
        .ok();
}

/// Enqueues a UI error into `state.errors` and schedules UI processing via `WM_APP_ERROR`.
///
/// Behavior:
/// - pushes a new `UiError` into the queue
/// - posts `WM_APP_ERROR` to the window to trigger UI side presentation
///
/// Deduplication:
/// If the last queued error has the same `title` and `user_text`, the new one is dropped.
/// This prevents UI spam for repeating failures.
pub fn push(
    hwnd: HWND,
    state: &mut AppState,
    title: &str,
    user_text: &str,
    err: &windows::core::Error,
) {
    if let Some(last) = state.errors.back()
        && last.title == title
        && last.user_text == user_text
    {
        return;
    }

    let debug_text = format!("{err:?}");

    state.errors.push_back(UiError {
        title: title.to_string(),
        user_text: user_text.to_string(),
        _debug_text: debug_text,
    });

    unsafe {
        if let Err(e) = PostMessageW(Some(hwnd), WM_APP_ERROR, WPARAM(0), LPARAM(0)) {
            tracing::warn!(error=?e, "PostMessageW(WM_APP_ERROR) failed");
        }
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
        $crate::platform::ui::error_notifier::report_unit($hwnd, $state, $title, $text, $expr);
    }};
}

/// Calls the provided closure or expression, stores the result in a temporary, then reports `Err`.
///
/// This is useful when the call is not a simple expression and you want a named `r` for debugging.
#[macro_export]
macro_rules! ui_call {
    ($hwnd:expr, $state:expr, $title:expr, $text:expr, $call:expr) => {{
        let r = $call;
        $crate::platform::ui::error_notifier::report_unit($hwnd, $state, $title, $text, r);
    }};
}
