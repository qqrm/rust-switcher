mod mapping;
use std::{ptr::null_mut, thread, time::Duration};

use mapping::convert_ru_en_bidirectional;
use windows::Win32::{
    Foundation::{HGLOBAL, HWND, LPARAM, WPARAM},
    System::{
        DataExchange::{
            CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
        },
        Memory::{GlobalLock, GlobalUnlock},
    },
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetKeyboardLayout, GetKeyboardLayoutList, HKL, INPUT, INPUT_0,
            INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_CONTROL,
            VK_LSHIFT, VK_RSHIFT,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
        },
    },
};

use crate::app::AppState;

const VK_C_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x43);
const VK_LEFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x25);
const VK_RIGHT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x27);
const VK_SHIFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x10);
const VK_DELETE_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x2E);

const CF_UNICODETEXT_ID: u32 = 13;

#[tracing::instrument(level = "trace", skip(state))]
pub fn convert_selection_if_any(state: &mut AppState) -> bool {
    let Some(s) = copy_selection_text_with_clipboard_restore(256) else {
        tracing::trace!("no selection");
        return false;
    };

    tracing::trace!(len = s.chars().count(), "selection detected");
    convert_selection_from_text(state, s);
    true
}

#[tracing::instrument(level = "trace", skip(state))]
pub fn convert_last_word(state: &mut AppState) {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        tracing::warn!("foreground window is null");
        return;
    }

    if !wait_shift_released(150) {
        tracing::info!("wait_shift_released returned false");
        return;
    }

    let delay_ms = crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100);
    tracing::trace!(delay_ms, "sleep before convert");
    std::thread::sleep(std::time::Duration::from_millis(delay_ms as u64));

    let Some((word, suffix)) = crate::input_journal::take_last_word_with_suffix() else {
        tracing::info!("journal: no last word");
        return;
    };

    let word_len = word.chars().count();
    let suffix_len = suffix.chars().count();
    tracing::trace!(%word, %suffix, word_len, suffix_len, "journal extracted");

    if word.is_empty() {
        tracing::warn!("journal returned empty word");
        return;
    }

    let converted = convert_ru_en_bidirectional(&word);
    tracing::trace!(%converted, "converted");

    let delete_count = word_len.saturating_add(suffix_len).min(4096);
    tracing::info!(delete_count, "delete_count computed");

    let mut seq = KeySequence::new();
    for i in 0..delete_count {
        if !seq.tap(VIRTUAL_KEY(0x08)) {
            tracing::error!(i, delete_count, "backspace tap failed");
            return;
        }
    }
    tracing::trace!("backspaces sent");

    if !send_text_unicode(&converted) {
        tracing::error!("send_text_unicode(converted) failed");
        return;
    }
    tracing::trace!("converted text sent");

    if !suffix.is_empty() {
        if !send_text_unicode(&suffix) {
            tracing::error!("send_text_unicode(suffix) failed");
            return;
        }
        tracing::trace!("suffix sent");
    }

    crate::input_journal::push_text(&converted);
    if !suffix.is_empty() {
        crate::input_journal::push_text(&suffix);
    }
    tracing::trace!("journal updated");

    match switch_keyboard_layout() {
        Ok(()) => tracing::trace!("layout switched"),
        Err(e) => tracing::warn!(error = ?e, "layout switch failed"),
    }
}

fn convert_selection_from_text(state: &mut AppState, text: String) {
    let delay_ms = crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100);

    let converted = convert_ru_en_bidirectional(&text);
    let converted_units = converted.encode_utf16().count();

    thread::sleep(Duration::from_millis(delay_ms as u64));

    let mut seq = KeySequence::new();

    if !seq.tap(VK_DELETE_KEY) {
        return;
    }

    if !send_text_unicode(&converted) {
        return;
    }

    thread::sleep(Duration::from_millis(20));

    let _ = reselect_last_inserted_text_utf16_units(converted_units);
    let _ = switch_keyboard_layout();
}

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

    let Some(text) = copy_selection_text_with_clipboard_restore(256) else {
        tracing::trace!("no selection");
        return;
    };

    convert_selection_from_text(state, text);
}

pub fn switch_keyboard_layout() -> windows::core::Result<()> {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return Ok(());
        }

        let tid = GetWindowThreadProcessId(fg, None);
        let cur = GetKeyboardLayout(tid);

        let n = GetKeyboardLayoutList(None);
        if n <= 0 {
            return Ok(());
        }

        let mut layouts = vec![HKL(null_mut()); n as usize];

        let n2 = GetKeyboardLayoutList(Some(layouts.as_mut_slice()));
        if n2 <= 0 {
            return Ok(());
        }
        layouts.truncate(n2 as usize);

        let next = next_layout(&layouts, cur);
        post_layout_change(fg, next)?;

        Ok(())
    }
}

fn next_layout(layouts: &[HKL], cur: HKL) -> HKL {
    if layouts.is_empty() {
        return cur;
    }

    let mut it = layouts.iter().copied().cycle();

    while let Some(h) = it.next() {
        if h == cur {
            return it.next().unwrap_or(cur);
        }

        if h == layouts[layouts.len() - 1] {
            return layouts[0];
        }
    }

    cur
}

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

fn send_ctrl_combo(vk: VIRTUAL_KEY) -> bool {
    let mut seq = KeySequence::new();

    if !seq.down(VK_CONTROL) {
        return false;
    }

    seq.tap(vk)
}

struct KeySequence {
    pressed: Vec<VIRTUAL_KEY>,
}

impl KeySequence {
    fn new() -> Self {
        Self {
            pressed: Vec::new(),
        }
    }

    fn down(&mut self, vk: VIRTUAL_KEY) -> bool {
        if send_key(vk, false) {
            self.pressed.push(vk);
            true
        } else {
            false
        }
    }

    fn tap(&mut self, vk: VIRTUAL_KEY) -> bool {
        send_key(vk, false) && send_key(vk, true)
    }
}

impl Drop for KeySequence {
    fn drop(&mut self) {
        for vk in self.pressed.drain(..).rev() {
            let _ = send_key(vk, true);
        }
    }
}

fn send_text_unicode(text: &str) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_UNICODE;

    let units: Vec<u16> = text.encode_utf16().collect();
    if units.is_empty() {
        return true;
    }

    let mut inputs = Vec::with_capacity(units.len() * 2);

    for u in units {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: u,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });

        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: u,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize;
    sent == inputs.len()
}

fn send_key(vk: VIRTUAL_KEY, key_up: bool) -> bool {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    sent != 0
}

fn reselect_last_inserted_text_utf16_units(units: usize) -> bool {
    if units == 0 {
        return true;
    }

    let units = units.min(4096);

    let mut seq = KeySequence::new();

    for _ in 0..units {
        if !seq.tap(VK_LEFT_KEY) {
            return false;
        }
    }

    if !seq.down(VK_SHIFT_KEY) {
        return false;
    }

    for _ in 0..units {
        if !seq.tap(VK_RIGHT_KEY) {
            return false;
        }
    }

    true
}

fn wait_shift_released(timeout_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);

    while std::time::Instant::now() < deadline {
        let l = unsafe { GetAsyncKeyState(VK_LSHIFT.0 as i32) } as u16;
        let r = unsafe { GetAsyncKeyState(VK_RSHIFT.0 as i32) } as u16;

        if (l & 0x8000) == 0 && (r & 0x8000) == 0 {
            return true;
        }

        thread::sleep(Duration::from_millis(1));
    }

    false
}

fn wait_clipboard_change(before: u32, tries: usize, sleep_ms: u64) -> bool {
    for _ in 0..tries {
        let now = unsafe { GetClipboardSequenceNumber() };
        if now != before {
            return true;
        }
        thread::sleep(Duration::from_millis(sleep_ms));
    }
    false
}

struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> Option<Self> {
        unsafe {
            OpenClipboard(None).ok()?;
        }
        Some(Self)
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

use windows::Win32::System::{
    DataExchange::{EmptyClipboard, SetClipboardData},
    Memory::{GMEM_MOVEABLE, GlobalAlloc},
};

fn clipboard_set_unicode_text(text: &str) -> bool {
    let _clip = match ClipboardGuard::open() {
        Some(g) => g,
        None => return false,
    };

    unsafe {
        let _ = EmptyClipboard();

        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);

        let bytes = units.len() * 2;

        let hmem = match GlobalAlloc(GMEM_MOVEABLE, bytes) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(error = ?e, "GlobalAlloc failed");
                return false;
            }
        };

        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            tracing::warn!("GlobalLock returned null");
            return false;
        }

        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
        let _ = GlobalUnlock(hmem);

        let handle = windows::Win32::Foundation::HANDLE(hmem.0);
        match SetClipboardData(CF_UNICODETEXT_ID, Some(handle)) {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(error = ?e, "SetClipboardData failed");
                false
            }
        }
    }
}

fn copy_selection_text_with_clipboard_restore(max_chars: usize) -> Option<String> {
    let old = clipboard_get_unicode_text();
    let before_seq = unsafe { GetClipboardSequenceNumber() };

    if !send_ctrl_combo(VK_C_KEY) {
        tracing::trace!("Ctrl+C failed to send");
        return None;
    }

    if !wait_clipboard_change(before_seq, 10, 20) {
        tracing::trace!("clipboard sequence did not change");
        return None;
    }

    let copied = clipboard_get_unicode_text().unwrap_or_default();

    if let Some(old_text) = old.as_deref() {
        let _ = clipboard_set_unicode_text(old_text);
    }

    if copied.is_empty() {
        return None;
    }

    if copied.contains('\n') || copied.contains('\r') {
        tracing::trace!("copied contains newline, reject");
        return None;
    }

    let len = copied.chars().count();
    if len > max_chars {
        tracing::trace!(len, max_chars, "copied too long, reject");
        return None;
    }

    Some(copied)
}
fn clipboard_get_unicode_text() -> Option<String> {
    let _clip = ClipboardGuard::open()?;

    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT_ID).ok()?;
        if handle.0.is_null() {
            return None;
        }

        let hglobal = HGLOBAL(handle.0);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            return None;
        }

        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        let slice = std::slice::from_raw_parts(ptr, len);
        let text = String::from_utf16_lossy(slice);

        let _ = GlobalUnlock(hglobal);
        Some(text)
    }
}
