use std::{ptr::null_mut, sync::Once, thread, time::Duration};

use windows::{
    Win32::{
        Foundation::{HGLOBAL, HWND, LPARAM, WPARAM},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            },
            DataExchange::{
                CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
            },
            Memory::{GlobalLock, GlobalUnlock},
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
            },
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, GetKeyboardLayout, GetKeyboardLayoutList, HKL, INPUT, INPUT_0,
                INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_CONTROL,
                VK_LSHIFT, VK_RSHIFT,
            },
            WindowsAndMessaging::{
                GetForegroundWindow, GetWindowThreadProcessId, PostMessageW,
                WM_INPUTLANGCHANGEREQUEST,
            },
        },
    },
    core::BSTR,
};

use crate::app::AppState;

const VK_C_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x43);
// const VK_V_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x56);
const VK_LEFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x25);
const VK_RIGHT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x27);
const VK_SHIFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x10);
const VK_DELETE_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x2E);

const CF_UNICODETEXT_ID: u32 = 13;

static COM_INIT: Once = Once::new();

enum UiaSelection {
    NoSelection,
    Text(String),
    HasSelectionButTextUnavailable,
    Unavailable,
}

fn uia_get_selection() -> UiaSelection {
    ensure_com_initialized();

    let uia: IUIAutomation =
        match unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok() {
            Some(v) => v,
            None => return UiaSelection::Unavailable,
        };

    let focused = match unsafe { uia.GetFocusedElement() }.ok() {
        Some(v) => v,
        None => return UiaSelection::Unavailable,
    };

    let tp: IUIAutomationTextPattern =
        match unsafe { focused.GetCurrentPatternAs(UIA_TextPatternId) }.ok() {
            Some(v) => v,
            None => return UiaSelection::Unavailable,
        };

    let ranges = match unsafe { tp.GetSelection() }.ok() {
        Some(v) => v,
        None => return UiaSelection::Unavailable,
    };

    let len = match unsafe { ranges.Length() }.ok() {
        Some(v) => v,
        None => return UiaSelection::Unavailable,
    };

    if len <= 0 {
        return UiaSelection::NoSelection;
    }

    let range = match unsafe { ranges.GetElement(0) }.ok() {
        Some(v) => v,
        None => return UiaSelection::HasSelectionButTextUnavailable,
    };

    let b: BSTR = match unsafe { range.GetText(-1) }.ok() {
        Some(v) => v,
        None => return UiaSelection::HasSelectionButTextUnavailable,
    };

    let s = b.to_string();
    if s.is_empty() {
        UiaSelection::HasSelectionButTextUnavailable
    } else {
        UiaSelection::Text(s)
    }
}

fn ensure_com_initialized() {
    COM_INIT.call_once(|| unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    });
}

pub fn convert_by_context(state: &mut AppState, hwnd: HWND) {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return;
    }

    if !wait_shift_released(150) {
        return;
    }

    match uia_get_selection() {
        UiaSelection::Text(text) => {
            convert_selection_from_text(state, text);
        }
        UiaSelection::HasSelectionButTextUnavailable => {
            convert_selection(state, hwnd);
        }
        UiaSelection::NoSelection | UiaSelection::Unavailable => {
            convert_last_word(state, hwnd);
        }
    }
}

pub fn convert_last_word(_state: &mut AppState, _hwnd: HWND) {
    let Some(word) = crate::input_journal::take_last_word() else {
        return;
    };
    if word.is_empty() {
        return;
    }

    let converted = convert_ru_en_bidirectional(&word);

    let mut seq = KeySequence::new();

    for _ in 0..word.chars().count().min(4096) {
        if !seq.tap(VIRTUAL_KEY(0x08)) {
            return;
        }
    }

    if !send_text_unicode(&converted) {
        return;
    }

    let _ = switch_keyboard_layout();
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

pub fn convert_selection(state: &mut AppState, _hwnd: HWND) {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return;
    }

    if !wait_shift_released(150) {
        return;
    }

    let before_seq = unsafe { GetClipboardSequenceNumber() };

    if !send_ctrl_combo(VK_C_KEY) {
        return;
    }

    if !wait_clipboard_change(before_seq, 10, 20) {
        return;
    }

    let Some(text) = clipboard_get_unicode_text() else {
        return;
    };

    if text.is_empty() {
        return;
    }

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

    true
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
