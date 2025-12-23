use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};

use windows::Win32::UI::{
    Input::KeyboardAndMouse::{
        GetKeyboardLayout, GetKeyboardState, MOD_ALT, MOD_CONTROL, ToUnicodeEx, VIRTUAL_KEY,
        VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_INSERT, VK_LEFT, VK_NEXT,
        VK_PRIOR, VK_RETURN, VK_RIGHT, VK_TAB, VK_UP,
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
}

impl InputJournal {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            buf: VecDeque::with_capacity(cap),
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

    fn take_last_word(&mut self) -> Option<String> {
        while matches!(self.buf.back(), Some(ch) if ch.is_whitespace()) {
            let _ = self.buf.pop_back();
        }

        let mut n = 0usize;
        for ch in self.buf.iter().rev() {
            if is_word_char(*ch) {
                n += 1;
            } else {
                break;
            }
        }

        if n == 0 {
            return None;
        }

        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(ch) = self.buf.pop_back() {
                out.push(ch);
            }
        }

        out.reverse();
        Some(out.into_iter().collect())
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
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

pub fn record_keydown(kb: &KBDLLHOOKSTRUCT, vk: u32) {
    if kb.flags.contains(LLKHF_INJECTED) {
        return;
    }

    let vk = VIRTUAL_KEY(vk as u16);

    match vk {
        VK_ESCAPE | VK_DELETE | VK_INSERT | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT => {
            if let Ok(mut j) = journal().lock() {
                j.clear();
            }
            return;
        }
        VK_BACK => {
            if let Ok(mut j) = journal().lock() {
                j.backspace();
            }
            return;
        }
        VK_RETURN => {
            if let Ok(mut j) = journal().lock() {
                j.push_str("\n");
            }
            return;
        }
        VK_TAB => {
            if let Ok(mut j) = journal().lock() {
                j.push_str("\t");
            }
            return;
        }
        _ => {}
    }

    if mods_ctrl_or_alt_down() {
        if let Ok(mut j) = journal().lock() {
            j.clear();
        }
        return;
    }

    let Some(s) = decode_typed_text(kb, vk) else {
        return;
    };

    if let Ok(mut j) = journal().lock() {
        j.push_str(&s);
    }
}

pub fn take_last_word() -> Option<String> {
    journal().lock().ok().and_then(|mut j| j.take_last_word())
}
