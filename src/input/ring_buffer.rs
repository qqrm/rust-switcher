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
    last_fg_hwnd: isize,
}

impl InputJournal {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            buf: VecDeque::with_capacity(cap),
            last_token_autoconverted: false,
            last_fg_hwnd: 0,
        }
    }

    fn clear(&mut self) {
        self.buf.clear();
        self.last_token_autoconverted = false;
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

    fn invalidate_if_foreground_changed(&mut self) {
        let fg = unsafe { GetForegroundWindow() };
        let raw = fg.0 as isize;
        if raw == 0 {
            self.clear();
            self.last_fg_hwnd = 0;
            return;
        }

        if self.last_fg_hwnd == 0 {
            self.last_fg_hwnd = raw;
            return;
        }

        if self.last_fg_hwnd != raw {
            self.clear();
            self.last_fg_hwnd = raw;
        }
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
        .is_some_and(|j| j.last_token_autoconverted)
}

fn mods_ctrl_or_alt_down() -> bool {
    let mods = crate::platform::win::keyboard::mods::mods_now();
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

    let async_down = |vk: VIRTUAL_KEY| -> bool {
        let v = unsafe { GetAsyncKeyState(i32::from(vk.0)) }.cast_unsigned();
        (v & 0x8000) != 0
    };

    let apply_async_key = |state: &mut [u8; 256], vk: VIRTUAL_KEY| {
        let idx = usize::from(vk.0);
        if idx >= state.len() {
            return;
        }

        if async_down(vk) {
            state[idx] |= 0x80;
        } else {
            state[idx] &= !0x80;
        }
    };

    apply_async_key(&mut state, VK_SHIFT);
    apply_async_key(&mut state, VK_LSHIFT);
    apply_async_key(&mut state, VK_RSHIFT);

    let mut buf = [0u16; 8];
    let rc = unsafe { ToUnicodeEx(u32::from(vk.0), kb.scanCode, &state, &mut buf, 0, Some(hkl)) };

    if rc == -1 {
        let _ =
            unsafe { ToUnicodeEx(u32::from(vk.0), kb.scanCode, &state, &mut buf, 0, Some(hkl)) };
        return None;
    }

    if rc <= 0 {
        return None;
    }

    let rc = usize::try_from(rc).ok()?;
    let s = String::from_utf16_lossy(&buf[..rc]);

    if s.chars().any(char::is_control) {
        return None;
    }

    Some(s)
}

pub fn record_keydown(kb: &KBDLLHOOKSTRUCT, vk: u32) -> Option<String> {
    if kb.flags.contains(LLKHF_INJECTED) {
        return None;
    }

    let vk_u16 = u16::try_from(vk).ok()?;
    let vk = VIRTUAL_KEY(vk_u16);

    enum JournalAction {
        Clear,
        Backspace,
        Push(String),
    }

    let mut action: Option<JournalAction> = None;
    let mut output: Option<String> = None;

    match vk {
        VK_ESCAPE | VK_DELETE | VK_INSERT | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT => action = Some(JournalAction::Clear),
        VK_BACK => action = Some(JournalAction::Backspace),
        VK_RETURN => {
            output = Some("\n".to_string());
            action = Some(JournalAction::Push("\n".to_string()));
        }
        VK_TAB => {
            output = Some("\t".to_string());
            action = Some(JournalAction::Push("\t".to_string()));
        }
        _ => {}
    }

    if mods_ctrl_or_alt_down() {
        action = Some(JournalAction::Clear);
    }

    if action.is_none() {
        let s = decode_typed_text(kb, vk)?;
        output = Some(s.clone());
        action = Some(JournalAction::Push(s));
    }

    if let Ok(mut j) = journal().lock() {
        j.invalidate_if_foreground_changed();
        if let Some(action) = action {
            match action {
                JournalAction::Clear => j.clear(),
                JournalAction::Backspace => j.backspace(),
                JournalAction::Push(s) => {
                    if s.chars().any(char::is_alphanumeric) {
                        j.last_token_autoconverted = false;
                    }
                    j.push_str(&s);
                }
            }
        }
    }

    output
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

pub fn invalidate() {
    if let Ok(mut j) = journal().lock() {
        j.clear();
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

    let Some(&last) = j.buf.back() else {
        return false;
    };

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
