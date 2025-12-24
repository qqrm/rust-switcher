mod clipboard;
use clipboard as clip;

mod input;
mod last_word;

pub use last_word::convert_last_word;

mod mapping;

use std::{ptr::null_mut, thread, time::Duration};

use mapping::convert_ru_en_bidirectional;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    System::DataExchange::GetClipboardSequenceNumber,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetKeyboardLayout, GetKeyboardLayoutList, HKL, VIRTUAL_KEY,
            VK_LSHIFT, VK_RSHIFT,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
        },
    },
};

use crate::{
    app::AppState,
    conversion::input::{
        KeySequence, reselect_last_inserted_text_utf16_units, send_ctrl_combo, send_text_unicode,
    },
};

/// Virtual key code for the `C` key.
///
/// Used together with Ctrl to trigger the standard Copy shortcut.
const VK_C_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x43);

/// Virtual key code for the Delete key.
///
/// Used to remove the current selection before inserting converted text.
const VK_DELETE_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x2E);

/// Attempts to convert the current selection by reading it through the clipboard.
///
/// This helper performs only the clipboard based selection acquisition:
/// - sends Ctrl+C to the foreground application
/// - waits for `GetClipboardSequenceNumber` to change
/// - reads Unicode text from the clipboard
/// - restores previous clipboard contents via `ClipboardRestore`
///
/// Return value:
/// - `None` if there is no eligible selection (empty, multiline, too long, or clipboard did not change)
/// - `Some(Ok(()))` if selection was converted successfully
/// - `Some(Err(ConvertSelectionError))` if selection was present but the conversion pipeline failed
///
/// Architectural note:
/// This function does not perform UI safety checks (foreground window, modifier keys).
/// Callers on the UI boundary should gate calls with `GetForegroundWindow` and `wait_shift_released`.
fn try_convert_selection_from_clipboard(
    state: &mut AppState,
    max_chars: usize,
) -> Option<std::result::Result<(), ConvertSelectionError>> {
    copy_selection_text_with_clipboard_restore(max_chars).map(|s| {
        tracing::trace!(len = s.chars().count(), "selection detected");
        convert_selection_from_text(state, &s)
    })
}

/// Converts the currently selected text, if there is any selection.
///
/// Returns `true` if a non empty eligible selection was found (conversion attempted),
/// otherwise `false`.
#[tracing::instrument(level = "trace", skip(state))]
pub fn convert_selection_if_any(state: &mut AppState) -> bool {
    match try_convert_selection_from_clipboard(state, 256) {
        None => {
            tracing::trace!("no selection");
            false
        }
        Some(Ok(())) => true,
        Some(Err(e)) => {
            tracing::warn!(user_text = e.user_text(), error = ?e, "selection conversion failed");
            true
        }
    }
}

/// Errors that can occur while replacing the current selection with converted text.
#[derive(Debug)]
enum ConvertSelectionError {
    /// Failed to send Delete to remove the current selection.
    Delete,
    /// Failed to inject Unicode text via `SendInput`.
    InsertConverted,
    /// Failed to reselect the inserted text within the retry budget.
    Reselect,
}

impl ConvertSelectionError {
    fn user_text(&self) -> &'static str {
        match self {
            Self::Delete => "Failed to delete selection",
            Self::InsertConverted => "Failed to insert converted text",
            Self::Reselect => "Failed to reselect inserted text",
        }
    }
}

/// Replaces currently selected text with layout converted text.
///
/// Returns `Ok(())` when:
/// - Delete tap succeeded
/// - Unicode injection succeeded
/// - reselect succeeded within retry budget
///
/// Keyboard layout switching is best effort and does not affect the result.
fn convert_selection_from_text(
    state: &mut AppState,
    text: &str,
) -> Result<(), ConvertSelectionError> {
    let delay_ms = crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100);

    let converted = convert_ru_en_bidirectional(text);
    let converted_units = converted.encode_utf16().count();

    thread::sleep(Duration::from_millis(delay_ms as u64));

    let mut seq = KeySequence::new();

    seq.tap(VK_DELETE_KEY)
        .then_some(())
        .ok_or(ConvertSelectionError::Delete)?;

    send_text_unicode(&converted)
        .then_some(())
        .ok_or(ConvertSelectionError::InsertConverted)?;

    reselect_with_retry(
        converted_units,
        Duration::from_millis(120),
        Duration::from_millis(5),
    )
    .then_some(())
    .ok_or(ConvertSelectionError::Reselect)?;

    if let Err(e) = switch_keyboard_layout() {
        tracing::trace!(error = ?e, "layout switch failed");
    }

    Ok(())
}

/// Attempts to reselect the last inserted text using bounded retries.
///
/// Rationale:
/// Some target applications update caret position and selection state asynchronously
/// relative to `SendInput`. A single fixed delay is either too long for fast apps or
/// too short for slow ones. Retrying for a short budget reduces median latency while
/// keeping behavior stable under load.
///
/// Returns `true` if reselect succeeds within `budget`, otherwise `false`.
fn reselect_with_retry(units: usize, budget: Duration, step_sleep: Duration) -> bool {
    let deadline = std::time::Instant::now() + budget;

    loop {
        if reselect_last_inserted_text_utf16_units(units) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        thread::sleep(step_sleep);
    }
}

/// Converts the current selection, if it exists.
///
/// This function performs UI safety checks (foreground window and Shift state)
/// and returns when no selection is available.
#[tracing::instrument(level = "trace", skip(state))]
pub fn convert_selection(state: &mut AppState) {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        tracing::warn!("foreground window is null");
        return;
    }

    if !wait_shift_released(150) {
        tracing::info!("wait_shift_released returned false");
        return;
    }

    match try_convert_selection_from_clipboard(state, 256) {
        None => tracing::trace!("no selection"),
        Some(Ok(())) => {}
        Some(Err(e)) => {
            tracing::warn!(user_text = e.user_text(), error = ?e, "selection conversion failed")
        }
    }
}

/// Returns the current foreground window, or `None` if it is null.
fn foreground_window() -> Option<HWND> {
    let fg = unsafe { GetForegroundWindow() };
    (!fg.0.is_null()).then_some(fg)
}

/// Returns the current keyboard layout for the thread owning `fg`.
fn current_layout_for_window(fg: HWND) -> HKL {
    unsafe {
        let tid = GetWindowThreadProcessId(fg, None);
        GetKeyboardLayout(tid)
    }
}

/// Enumerates installed keyboard layouts for the current desktop.
///
/// Returns an empty vector when enumeration fails or yields no results.
fn installed_layouts() -> Vec<HKL> {
    unsafe {
        let n = GetKeyboardLayoutList(None);
        if n <= 0 {
            return Vec::new();
        }

        let mut layouts = vec![HKL(null_mut()); n as usize];
        let n2 = GetKeyboardLayoutList(Some(layouts.as_mut_slice()));
        if n2 <= 0 {
            return Vec::new();
        }

        layouts.truncate(n2 as usize);
        layouts
    }
}

/// Switches the keyboard layout for the current foreground window to the next installed layout.
///
/// Algorithm:
/// - obtains the foreground window
/// - reads the current layout for that window thread
/// - enumerates installed layouts
/// - selects the next one cyclically
/// - posts `WM_INPUTLANGCHANGEREQUEST` to the window
///
/// Returns `Ok(())` when the operation is completed or skipped:
/// - no foreground window
/// - no layouts available
///
/// Returns `Err` only if posting the request fails.
pub fn switch_keyboard_layout() -> windows::core::Result<()> {
    let Some(fg) = foreground_window() else {
        return Ok(());
    };

    let cur = current_layout_for_window(fg);
    let layouts = installed_layouts();
    if layouts.is_empty() {
        return Ok(());
    }

    let next = next_layout(&layouts, cur);
    post_layout_change(fg, next)
}

/// Returns the next layout in `layouts` after `cur`, cycling back to the first.
///
/// If `cur` is not found, returns `cur`.
fn next_layout(layouts: &[HKL], cur: HKL) -> HKL {
    layouts
        .iter()
        .position(|&h| h == cur)
        .and_then(|i| layouts.get((i + 1) % layouts.len()).copied())
        .unwrap_or(cur)
}

/// Posts a layout change request message to the foreground window.
///
/// Uses `WM_INPUTLANGCHANGEREQUEST`. The `hkl` is passed through `LPARAM`.
fn post_layout_change(fg: HWND, hkl: HKL) -> windows::core::Result<()> {
    unsafe {
        PostMessageW(
            Some(fg),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(hkl.0 as isize),
        )?;
    }
    Ok(())
}

/// Waits until both left and right Shift keys are released or the timeout elapses.
///
/// Returns `true` as soon as neither Shift key is currently pressed.
fn wait_shift_released(timeout_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);

    std::iter::from_fn(|| {
        let now = std::time::Instant::now();
        (now < deadline).then_some(())
    })
    .any(|_| {
        let l = unsafe { GetAsyncKeyState(VK_LSHIFT.0 as i32) } as u16;
        let r = unsafe { GetAsyncKeyState(VK_RSHIFT.0 as i32) } as u16;

        let released = (l & 0x8000) == 0 && (r & 0x8000) == 0;
        if released {
            return true;
        }

        thread::sleep(Duration::from_millis(1));
        false
    })
}

/// RAII helper that restores clipboard Unicode text on drop.
///
/// The conversion pipeline uses the clipboard for reading the current selection.
/// This guard preserves the previous clipboard text and restores it even on early
/// returns or errors.
struct ClipboardRestore {
    old: Option<String>,
}

impl ClipboardRestore {
    /// Captures current clipboard Unicode text.
    fn capture() -> Self {
        Self {
            old: clip::get_unicode_text(),
        }
    }
}

impl Drop for ClipboardRestore {
    fn drop(&mut self) {
        if let Some(old_text) = self.old.as_deref() {
            let _ = clip::set_unicode_text(old_text);
        }
    }
}

/// Copies current selection via Ctrl+C, reads Unicode text from clipboard, then restores clipboard.
///
/// Returns `None` when selection is empty, multiline, too long, or clipboard did not change.
/// `max_chars` is counted in Unicode scalar values.
fn copy_selection_text_with_clipboard_restore(max_chars: usize) -> Option<String> {
    let _restore = ClipboardRestore::capture();
    let before_seq = unsafe { GetClipboardSequenceNumber() };

    send_ctrl_combo(VK_C_KEY)
        .then(|| clip::wait_change(before_seq, 10, 20))
        .filter(|&changed| changed)
        .and_then(|_| clip::get_unicode_text())
        .filter(|s| is_convertible_selection(s, max_chars))
}

/// Checks whether clipboard text is eligible for selection conversion.
///
/// Optimization:
/// `s.chars().nth(max_chars).is_none()` stops early for long strings, unlike `chars().count()`.
fn is_convertible_selection(s: &str, max_chars: usize) -> bool {
    !s.is_empty() && !s.contains('\n') && !s.contains('\r') && s.chars().nth(max_chars).is_none()
}
