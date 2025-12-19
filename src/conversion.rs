use std::{ptr::null_mut, thread, time::Duration};

use crate::app::AppState;
use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL, HWND, LPARAM, WPARAM},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
            OpenClipboard, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyboardLayout, GetKeyboardLayoutList, HKL, INPUT, INPUT_0, INPUT_KEYBOARD,
            KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_CONTROL,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
        },
    },
};

const VK_C_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x43);

const VK_V_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x56);
const CF_UNICODETEXT_ID: u32 = 13;

pub fn convert_last_word(state: &mut AppState, hwnd: HWND) {
    convert_selection(state, hwnd);
}

pub fn convert_selection(state: &mut AppState, _hwnd: HWND) {
    let delay_ms = unsafe { crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100) };

    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return;
        }

        let before_seq = GetClipboardSequenceNumber();

        if !send_ctrl_combo(VK_C_KEY) {
            return;
        }

        if !wait_clipboard_change(before_seq, 10, 20) {
            return;
        }

        let Some(text) = clipboard_get_unicode_text() else {
            return;
        };

        thread::sleep(Duration::from_millis(delay_ms as u64));

        let _ = switch_keyboard_layout();

        if clipboard_set_unicode_text(&text).is_none() {
            return;
        }

        let _ = send_ctrl_combo(VK_V_KEY);
    }
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

unsafe fn send_ctrl_combo(vk: VIRTUAL_KEY) -> bool {
    (unsafe { send_key(VK_CONTROL, false) })
        && unsafe { send_key(vk, false) }
        && unsafe { send_key(vk, true) }
        && unsafe { send_key(VK_CONTROL, true) }
}

unsafe fn send_key(vk: VIRTUAL_KEY, key_up: bool) -> bool {
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

    (unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) }) != 0
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

fn clipboard_get_unicode_text() -> Option<String> {
    unsafe {
        OpenClipboard(None).ok()?;

        let handle = GetClipboardData(CF_UNICODETEXT_ID).ok()?;
        if handle.0.is_null() {
            let _ = CloseClipboard();
            return None;
        }

        let hglobal = HGLOBAL(handle.0);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return None;
        }

        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        let slice = std::slice::from_raw_parts(ptr, len);
        let text = String::from_utf16_lossy(slice);

        let _ = GlobalUnlock(hglobal);
        let _ = CloseClipboard();
        Some(text)
    }
}

fn clipboard_set_unicode_text(text: &str) -> Option<()> {
    unsafe {
        OpenClipboard(None).ok()?;
        EmptyClipboard().ok()?;

        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);

        let bytes = wide.len() * 2;
        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes).ok()?;

        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return None;
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());

        let _ = GlobalUnlock(hmem);

        SetClipboardData(CF_UNICODETEXT_ID, Some(HANDLE(hmem.0))).ok()?;

        let _ = CloseClipboard();
        Some(())
    }
}
