use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};

use windows::Win32::UI::{
    Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetKeyboardLayout, GetKeyboardState, MOD_ALT, MOD_CONTROL, ToUnicodeEx,
        VIRTUAL_KEY, VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_INSERT, VK_LEFT,
        VK_LSHIFT, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RSHIFT, VK_SHIFT, VK_TAB, VK_UP,
    },
    WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
    },
};

static JOURNAL: OnceLock<Mutex<InputJournal>> = OnceLock::new();

fn journal() -> &'static Mutex<InputJournal> {
    JOURNAL.get_or_init(|| Mutex::new(InputJournal::new(100)))
}

#[derive(Debug, Default)]
struct InputJournal {
    cap: usize,
    buf: VecDeque<char>,
    last_token_autoconverted: bool,
}

impl InputJournal {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            buf: VecDeque::with_capacity(cap),
            last_token_autoconverted: false,
        }
    }

    fn clear(&mut self) {
        self.buf.clear();
    }

    fn push_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.buf.push_back(ch);
        }
        while self.buf.len() > self.cap {
            let _ = self.buf.pop_front();
        }
    }

    fn backspace(&mut self) {
        let _ = self.buf.pop_back();
    }
}

pub fn mark_last_token_autoconverted() {
    if let Ok(mut j) = journal().lock() {
        j.last_token_autoconverted = true;
    }
}

pub fn last_token_autoconverted() -> bool {
    journal()
        .lock()
        .ok()
        .map(|j| j.last_token_autoconverted)
        .unwrap_or(false)
}

fn mods_ctrl_or_alt_down() -> bool {
    let mods = crate::win::keyboard::mods::mods_now();
    (mods & (MOD_CONTROL.0 | MOD_ALT.0)) != 0
}

fn decode_typed_text(kb: &KBDLLHOOKSTRUCT, vk: VIRTUAL_KEY) -> Option<String> {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return None;
    }

    let tid = unsafe { GetWindowThreadProcessId(fg, None) };
    let hkl = unsafe { GetKeyboardLayout(tid) };

    let mut state = [0u8; 256];
    if unsafe { GetKeyboardState(&mut state) }.is_err() {
        return None;
    }

    fn async_down(vk: VIRTUAL_KEY) -> bool {
        let v = unsafe { GetAsyncKeyState(vk.0 as i32) } as u16;
        (v & 0x8000) != 0
    }

    fn apply_async_key(state: &mut [u8; 256], vk: VIRTUAL_KEY) {
        let idx = vk.0 as usize;
        if idx >= state.len() {
            return;
        }

        if async_down(vk) {
            state[idx] |= 0x80;
        } else {
            state[idx] &= !0x80;
        }
    }

    apply_async_key(&mut state, VK_SHIFT);
    apply_async_key(&mut state, VK_LSHIFT);
    apply_async_key(&mut state, VK_RSHIFT);

    let mut buf = [0u16; 8];
    let rc = unsafe { ToUnicodeEx(vk.0 as u32, kb.scanCode, &state, &mut buf, 0, Some(hkl)) };

    if rc == -1 {
        let _ = unsafe { ToUnicodeEx(vk.0 as u32, kb.scanCode, &state, &mut buf, 0, Some(hkl)) };
        return None;
    }

    if rc <= 0 {
        return None;
    }

    let s = String::from_utf16_lossy(&buf[..rc as usize]);
    if s.chars().any(|c| c.is_control()) {
        return None;
    }

    Some(s)
}

pub fn record_keydown(kb: &KBDLLHOOKSTRUCT, vk: u32) -> Option<String> {
    if kb.flags.contains(LLKHF_INJECTED) {
        return None;
    }

    let vk = VIRTUAL_KEY(vk as u16);

    match vk {
        VK_ESCAPE | VK_DELETE | VK_INSERT | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT => {
            if let Ok(mut j) = journal().lock() {
                j.clear();
            }
            return None;
        }
        VK_BACK => {
            if let Ok(mut j) = journal().lock() {
                j.backspace();
            }
            return None;
        }
        VK_RETURN => {
            if let Ok(mut j) = journal().lock() {
                j.push_str("\n");
            }
            return Some("\n".to_string());
        }
        VK_TAB => {
            if let Ok(mut j) = journal().lock() {
                j.push_str("\t");
            }
            return Some("\t".to_string());
        }
        _ => {}
    }

    if mods_ctrl_or_alt_down() {
        if let Ok(mut j) = journal().lock() {
            j.clear();
        }
        return None;
    }

    let s = decode_typed_text(kb, vk)?;

    if s.chars().any(|c| c.is_alphanumeric())
        && let Ok(mut j) = journal().lock()
    {
        j.last_token_autoconverted = false;
    }

    if let Ok(mut j) = journal().lock() {
        j.push_str(&s);
    }

    Some(s)
}

pub fn take_last_word_with_suffix() -> Option<(String, String)> {
    let Ok(mut j) = journal().lock() else {
        return None;
    };

    // Trailing whitespace is suffix (spaces, tabs, newlines, etc).
    let mut suffix: Vec<char> = Vec::new();
    while let Some(&ch) = j.buf.back() {
        if ch.is_whitespace() {
            suffix.push(j.buf.pop_back()?);
        } else {
            break;
        }
    }

    // Token is the last contiguous run of non-whitespace characters.
    let mut token: Vec<char> = Vec::new();
    while let Some(&ch) = j.buf.back() {
        if ch.is_whitespace() {
            break;
        }
        token.push(j.buf.pop_back()?);
    }

    if token.is_empty() {
        // Restore suffix if we didn't get a token.
        while let Some(ch) = suffix.pop() {
            j.buf.push_back(ch);
        }
        return None;
    }

    token.reverse();
    suffix.reverse();

    Some((token.into_iter().collect(), suffix.into_iter().collect()))
}

pub fn push_text(s: &str) {
    if let Ok(mut j) = journal().lock() {
        j.push_str(s);
    }
}

pub fn last_char_triggers_autoconvert() -> bool {
    let Ok(j) = journal().lock() else {
        return false;
    };

    let len = j.buf.len();
    if len == 0 {
        return false;
    }

    let last = *j.buf.back().unwrap();

    // Trigger on punctuation immediately following a non-whitespace char.
    if matches!(last, '.' | ',' | '!' | '?' | ';' | ':') {
        if len < 2 {
            return false;
        }
        let prev = j.buf[len - 2];
        return !prev.is_whitespace();
    }

    // Trigger on the first whitespace after a non-whitespace run.
    if last.is_whitespace() {
        if len < 2 {
            return false;
        }
        let prev = j.buf[len - 2];
        return !prev.is_whitespace();
    }

    false
}
