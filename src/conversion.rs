use std::{ptr::null_mut, thread, time::Duration};

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

use crate::app::AppState;

const VK_C_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x43);
const VK_V_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x56);
const VK_LEFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x25);
const VK_RIGHT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x27);
const VK_SHIFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x10);
const CF_UNICODETEXT_ID: u32 = 13;

pub fn convert_last_word(state: &mut AppState, hwnd: HWND) {
    convert_selection(state, hwnd)
}

pub fn convert_selection(state: &mut AppState, _hwnd: HWND) {
    let delay_ms = crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100);

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

        let converted = convert_ru_en_bidirectional(&text);
        let converted_units = converted.encode_utf16().count();

        thread::sleep(Duration::from_millis(delay_ms as u64));

        if clipboard_set_unicode_text(&converted).is_none() {
            return;
        }

        if !send_ctrl_combo(VK_V_KEY) {
            return;
        }

        thread::sleep(Duration::from_millis(20));

        let _ = reselect_last_inserted_text_utf16_units(converted_units);

        let _ = switch_keyboard_layout();
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

unsafe fn reselect_last_inserted_text_utf16_units(units: usize) -> bool {
    if units == 0 {
        return true;
    }

    let units = units.min(4096);

    // Move caret to start of inserted text (no selection)
    for _ in 0..units {
        if !(unsafe { send_key(VK_LEFT_KEY, false) && send_key(VK_LEFT_KEY, true) }) {
            return false;
        }
    }

    // Select forward so that active end is on the right (caret ends at the end)
    if !unsafe { send_key(VK_SHIFT_KEY, false) } {
        return false;
    }

    let mut ok = true;
    for _ in 0..units {
        if !(unsafe { send_key(VK_RIGHT_KEY, false) && send_key(VK_RIGHT_KEY, true) }) {
            ok = false;
            break;
        }
    }

    let _ = unsafe { send_key(VK_SHIFT_KEY, true) };
    ok
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

fn clipboard_set_unicode_text(text: &str) -> Option<()> {
    let _clip = ClipboardGuard::open()?;

    unsafe {
        EmptyClipboard().ok()?;

        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * std::mem::size_of::<u16>();

        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes).ok()?;
        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            return None;
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());

        let _ = GlobalUnlock(hmem);

        SetClipboardData(CF_UNICODETEXT_ID, Some(HANDLE(hmem.0))).ok()?;
        Some(())
    }
}

fn convert_ru_en_bidirectional(text: &str) -> String {
    fn map_char(c: char) -> Option<char> {
        // EN -> RU
        #[rustfmt::skip]
        let mapped = match c {
            'q' => 'й', 'w' => 'ц', 'e' => 'у', 'r' => 'к', 't' => 'е', 'y' => 'н', 'u' => 'г', 'i' => 'ш', 'o' => 'щ', 'p' => 'з',
            '[' => 'х', ']' => 'ъ',
            'a' => 'ф', 's' => 'ы', 'd' => 'в', 'f' => 'а', 'g' => 'п', 'h' => 'р', 'j' => 'о', 'k' => 'л', 'l' => 'д',
            ';' => 'ж', '\'' => 'э',
            'z' => 'я', 'x' => 'ч', 'c' => 'с', 'v' => 'м', 'b' => 'и', 'n' => 'т', 'm' => 'ь',
            ',' => 'б', '.' => 'ю',
            '`' => 'ё',

            'Q' => 'Й', 'W' => 'Ц', 'E' => 'У', 'R' => 'К', 'T' => 'Е', 'Y' => 'Н', 'U' => 'Г', 'I' => 'Ш', 'O' => 'Щ', 'P' => 'З',
            '{' => 'Х', '}' => 'Ъ',
            'A' => 'Ф', 'S' => 'Ы', 'D' => 'В', 'F' => 'А', 'G' => 'П', 'H' => 'Р', 'J' => 'О', 'K' => 'Л', 'L' => 'Д',
            ':' => 'Ж', '"' => 'Э',
            'Z' => 'Я', 'X' => 'Ч', 'C' => 'С', 'V' => 'М', 'B' => 'И', 'N' => 'Т', 'M' => 'Ь',
            '<' => 'Б', '>' => 'Ю',
            '~' => 'Ё',
            _ => return None,
        };
        Some(mapped)
    }

    fn map_char_reverse(c: char) -> Option<char> {
        // RU -> EN
        #[rustfmt::skip]
        let mapped = match c {
            'й' => 'q', 'ц' => 'w', 'у' => 'e', 'к' => 'r', 'е' => 't', 'н' => 'y', 'г' => 'u', 'ш' => 'i', 'щ' => 'o', 'з' => 'p',
            'х' => '[', 'ъ' => ']',
            'ф' => 'a', 'ы' => 's', 'в' => 'd', 'а' => 'f', 'п' => 'g', 'р' => 'h', 'о' => 'j', 'л' => 'k', 'д' => 'l',
            'ж' => ';', 'э' => '\'',
            'я' => 'z', 'ч' => 'x', 'с' => 'c', 'м' => 'v', 'и' => 'b', 'т' => 'n', 'ь' => 'm',
            'б' => ',', 'ю' => '.',
            'ё' => '`',

            'Й' => 'Q', 'Ц' => 'W', 'У' => 'E', 'К' => 'R', 'Е' => 'T', 'Н' => 'Y', 'Г' => 'U', 'Ш' => 'I', 'Щ' => 'O', 'З' => 'P',
            'Х' => '{', 'Ъ' => '}',
            'Ф' => 'A', 'Ы' => 'S', 'В' => 'D', 'А' => 'F', 'П' => 'G', 'Р' => 'H', 'О' => 'J', 'Л' => 'K', 'Д' => 'L',
            'Ж' => ':', 'Э' => '"',
            'Я' => 'Z', 'Ч' => 'X', 'С' => 'C', 'М' => 'V', 'И' => 'B', 'Т' => 'N', 'Ь' => 'M',
            'Б' => '<', 'Ю' => '>',
            'Ё' => '~',
            _ => return None,
        };
        Some(mapped)
    }

    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if let Some(m) = map_char(ch) {
            out.push(m);
            continue;
        }
        if let Some(m) = map_char_reverse(ch) {
            out.push(m);
            continue;
        }
        out.push(ch);
    }
    out
}
