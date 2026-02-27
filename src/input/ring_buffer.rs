use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};

#[cfg(windows)]
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetKeyboardLayout, GetKeyboardState, HKL, ToUnicodeEx, VIRTUAL_KEY,
        VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_INSERT, VK_LEFT, VK_LSHIFT,
        VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RSHIFT, VK_SHIFT, VK_TAB, VK_UP,
    },
    WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
    },
};

static JOURNAL: OnceLock<Mutex<InputJournal>> = OnceLock::new();

fn journal() -> &'static Mutex<InputJournal> {
    JOURNAL.get_or_init(|| Mutex::new(InputJournal::new(100)))
}

#[cfg(windows)]
const LANG_ENGLISH_PRIMARY: u16 = 0x09;
#[cfg(windows)]
const LANG_RUSSIAN_PRIMARY: u16 = 0x19;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutTag {
    Ru,
    En,
    Other(u16),
    Unknown,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RunOrigin {
    Physical,
    Programmatic,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RunKind {
    Text,
    Whitespace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputRun {
    pub text: String,
    pub layout: LayoutTag,
    pub origin: RunOrigin,
    pub kind: RunKind,
}

#[derive(Debug, Default)]
struct InputJournal {
    runs: VecDeque<InputRun>,
    cap_chars: usize,
    total_chars: usize,
    last_token_autoconverted: bool,
    #[cfg(windows)]
    last_fg_hwnd: isize,
}

impl InputJournal {
    fn new(cap_chars: usize) -> Self {
        Self {
            runs: VecDeque::new(),
            cap_chars,
            total_chars: 0,
            last_token_autoconverted: false,
            #[cfg(windows)]
            last_fg_hwnd: 0,
        }
    }

    #[cfg(any(test, windows))]
    fn clear(&mut self) {
        self.runs.clear();
        self.total_chars = 0;
        self.last_token_autoconverted = false;
    }

    fn append_segment(&mut self, text: &str, layout: LayoutTag, origin: RunOrigin, kind: RunKind) {
        if text.is_empty() {
            return;
        }

        if let Some(last) = self.runs.back_mut()
            && last.layout == layout
            && last.origin == origin
            && last.kind == kind
        {
            last.text.push_str(text);
            self.total_chars += text.chars().count();
            self.enforce_cap_chars();
            return;
        }

        self.total_chars += text.chars().count();
        self.runs.push_back(InputRun {
            text: text.to_string(),
            layout,
            origin,
            kind,
        });
        self.enforce_cap_chars();
    }

    #[cfg(any(test, windows))]
    fn push_text_internal(&mut self, text: &str, layout: LayoutTag, origin: RunOrigin) {
        let mut segment = String::new();
        let mut segment_kind: Option<RunKind> = None;

        for ch in text.chars() {
            let kind = if ch.is_whitespace() {
                RunKind::Whitespace
            } else {
                RunKind::Text
            };

            match segment_kind {
                Some(current) if current == kind => segment.push(ch),
                Some(current) => {
                    self.append_segment(&segment, layout.clone(), origin, current);
                    segment.clear();
                    segment.push(ch);
                    segment_kind = Some(kind);
                }
                None => {
                    segment.push(ch);
                    segment_kind = Some(kind);
                }
            }
        }

        if let Some(kind) = segment_kind {
            self.append_segment(&segment, layout, origin, kind);
        }
    }

    fn push_run(&mut self, run: InputRun) {
        self.append_segment(&run.text, run.layout, run.origin, run.kind);
    }

    fn push_runs(&mut self, runs: impl IntoIterator<Item = InputRun>) {
        for run in runs {
            self.push_run(run);
        }
    }

    fn enforce_cap_chars(&mut self) {
        while self.total_chars > self.cap_chars {
            let mut remove_front_run = false;

            if let Some(front) = self.runs.front_mut() {
                if let Some((idx, _)) = front.text.char_indices().nth(1) {
                    front.text.drain(..idx);
                } else {
                    front.text.clear();
                    remove_front_run = true;
                }
                self.total_chars = self.total_chars.saturating_sub(1);

                if front.text.is_empty() {
                    remove_front_run = true;
                }
            } else {
                self.total_chars = 0;
                break;
            }

            if remove_front_run {
                let _ = self.runs.pop_front();
            }
        }
    }

    #[cfg(any(test, windows))]
    fn backspace(&mut self) {
        let mut pop_last = false;

        if let Some(last) = self.runs.back_mut()
            && let Some((idx, _)) = last.text.char_indices().last()
        {
            last.text.drain(idx..);
            self.total_chars = self.total_chars.saturating_sub(1);
            if last.text.is_empty() {
                pop_last = true;
            }
        }

        if pop_last {
            let _ = self.runs.pop_back();
        }
    }

    #[cfg(windows)]
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

    #[cfg(any(test, windows))]
    fn last_char(&self) -> Option<char> {
        self.runs.back()?.text.chars().last()
    }

    #[cfg(any(test, windows))]
    fn prev_char_before_last(&self) -> Option<char> {
        let mut runs_it = self.runs.iter().rev();
        let last_run = runs_it.next()?;

        let mut chars = last_run.text.chars().rev();
        let _ = chars.next()?;
        if let Some(prev) = chars.next() {
            return Some(prev);
        }

        for run in runs_it {
            if let Some(ch) = run.text.chars().last() {
                return Some(ch);
            }
        }

        None
    }

    fn take_last_layout_run_with_suffix(&mut self) -> Option<(InputRun, Vec<InputRun>)> {
        let mut suffix_runs: Vec<InputRun> = Vec::new();
        while self
            .runs
            .back()
            .is_some_and(|run| run.kind == RunKind::Whitespace)
        {
            let run = self.runs.pop_back()?;
            self.total_chars = self.total_chars.saturating_sub(run.text.chars().count());
            suffix_runs.push(run);
        }

        if self.runs.back().is_none_or(|run| run.kind != RunKind::Text) {
            while let Some(run) = suffix_runs.pop() {
                self.total_chars += run.text.chars().count();
                self.runs.push_back(run);
            }
            return None;
        }

        let run = self.runs.pop_back()?;
        self.total_chars = self.total_chars.saturating_sub(run.text.chars().count());
        suffix_runs.reverse();
        Some((run, suffix_runs))
    }

    fn take_last_layout_sequence_with_suffix(&mut self) -> Option<(Vec<InputRun>, Vec<InputRun>)> {
        let mut suffix_runs: Vec<InputRun> = Vec::new();
        while self
            .runs
            .back()
            .is_some_and(|run| run.kind == RunKind::Whitespace)
        {
            let run = self.runs.pop_back()?;
            self.total_chars = self.total_chars.saturating_sub(run.text.chars().count());
            suffix_runs.push(run);
        }

        if self.runs.back().is_none_or(|run| run.kind != RunKind::Text) {
            while let Some(run) = suffix_runs.pop() {
                self.total_chars += run.text.chars().count();
                self.runs.push_back(run);
            }
            return None;
        }

        let last = self.runs.back()?.clone();
        let target_layout = last.layout.clone();
        let target_origin = last.origin;
        let mut seq_rev: Vec<InputRun> = Vec::new();
        while let Some(run) = self.runs.back() {
            if run.layout != target_layout || run.origin != target_origin {
                break;
            }
            let run = self.runs.pop_back()?;
            self.total_chars = self.total_chars.saturating_sub(run.text.chars().count());
            seq_rev.push(run);
        }

        if seq_rev.is_empty() {
            while let Some(run) = suffix_runs.pop() {
                self.total_chars += run.text.chars().count();
                self.runs.push_back(run);
            }
            return None;
        }

        seq_rev.reverse();
        suffix_runs.reverse();
        Some((seq_rev, suffix_runs))
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct DecodedText {
    text: String,
    layout: LayoutTag,
}

#[cfg(windows)]
pub fn layout_tag_from_hkl(hkl: HKL) -> LayoutTag {
    let hkl_raw = hkl.0 as usize;

    if hkl_raw == 0 {
        return LayoutTag::Unknown;
    }

    let lang_id = (hkl_raw & 0xFFFF) as u16;
    let primary = lang_id & 0x03FF;

    match primary {
        LANG_ENGLISH_PRIMARY => LayoutTag::En,
        LANG_RUSSIAN_PRIMARY => LayoutTag::Ru,
        _ => LayoutTag::Other(lang_id),
    }
}

#[cfg(windows)]
fn current_foreground_layout_tag() -> LayoutTag {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return LayoutTag::Unknown;
    }

    let tid = unsafe { GetWindowThreadProcessId(fg, None) };
    let hkl = unsafe { GetKeyboardLayout(tid) };
    layout_tag_from_hkl(hkl)
}

pub fn mark_last_token_autoconverted() {
    if let Ok(mut j) = journal().lock() {
        j.last_token_autoconverted = true;
    }
}

#[cfg(any(test, windows))]
pub fn last_token_autoconverted() -> bool {
    journal()
        .lock()
        .ok()
        .is_some_and(|j| j.last_token_autoconverted)
}

#[cfg(windows)]
fn mods_ctrl_or_alt_down() -> bool {
    // Keep this module independent from `crate::platform` so it can be built from the minimal lib target.
    // VK_CONTROL = 0x11, VK_MENU (Alt) = 0x12.
    let ctrl = unsafe { GetAsyncKeyState(0x11) }.cast_unsigned();
    let alt = unsafe { GetAsyncKeyState(0x12) }.cast_unsigned();
    (ctrl & 0x8000) != 0 || (alt & 0x8000) != 0
}

#[cfg(windows)]
fn decode_typed_text(kb: &KBDLLHOOKSTRUCT, vk: VIRTUAL_KEY) -> Option<DecodedText> {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return None;
    }

    let tid = unsafe { GetWindowThreadProcessId(fg, None) };
    let hkl = unsafe { GetKeyboardLayout(tid) };
    let layout = layout_tag_from_hkl(hkl);

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

    Some(DecodedText { text: s, layout })
}

#[cfg(windows)]
pub fn record_keydown(kb: &KBDLLHOOKSTRUCT, vk: u32) -> Option<String> {
    if kb.flags.contains(LLKHF_INJECTED) {
        return None;
    }

    let vk_u16 = u16::try_from(vk).ok()?;
    let vk = VIRTUAL_KEY(vk_u16);

    enum JournalAction {
        Clear,
        Backspace,
        PushText {
            text: String,
            layout: LayoutTag,
            origin: RunOrigin,
        },
    }

    let mut action: Option<JournalAction> = None;
    let mut output: Option<String> = None;

    match vk {
        VK_ESCAPE | VK_DELETE | VK_INSERT | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT => action = Some(JournalAction::Clear),
        VK_BACK => action = Some(JournalAction::Backspace),
        VK_RETURN => {
            let layout = current_foreground_layout_tag();
            output = Some("\n".to_string());
            action = Some(JournalAction::PushText {
                text: "\n".to_string(),
                layout,
                origin: RunOrigin::Physical,
            });
        }
        VK_TAB => {
            let layout = current_foreground_layout_tag();
            output = Some("\t".to_string());
            action = Some(JournalAction::PushText {
                text: "\t".to_string(),
                layout,
                origin: RunOrigin::Physical,
            });
        }
        _ => {}
    }

    if mods_ctrl_or_alt_down() {
        action = Some(JournalAction::Clear);
    }

    if action.is_none() {
        let decoded = decode_typed_text(kb, vk)?;
        output = Some(decoded.text.clone());
        action = Some(JournalAction::PushText {
            text: decoded.text,
            layout: decoded.layout,
            origin: RunOrigin::Physical,
        });
    }

    if let Ok(mut j) = journal().lock() {
        j.invalidate_if_foreground_changed();
        if let Some(action) = action {
            match action {
                JournalAction::Clear => j.clear(),
                JournalAction::Backspace => j.backspace(),
                JournalAction::PushText {
                    text,
                    layout,
                    origin,
                } => {
                    if text.chars().any(char::is_alphanumeric) {
                        j.last_token_autoconverted = false;
                    }
                    j.push_text_internal(&text, layout, origin);
                }
            }
        }
    }

    output
}

pub fn take_last_layout_run_with_suffix() -> Option<(InputRun, Vec<InputRun>)> {
    journal().lock().ok()?.take_last_layout_run_with_suffix()
}

pub fn take_last_layout_sequence_with_suffix() -> Option<(Vec<InputRun>, Vec<InputRun>)> {
    journal()
        .lock()
        .ok()?
        .take_last_layout_sequence_with_suffix()
}

#[cfg(test)]
pub fn push_text(s: &str) {
    if let Ok(mut j) = journal().lock() {
        j.push_text_internal(s, LayoutTag::Unknown, RunOrigin::Programmatic);
    }
}

pub fn push_run(run: InputRun) {
    if let Ok(mut j) = journal().lock() {
        j.push_run(run);
    }
}

pub fn push_runs(runs: impl IntoIterator<Item = InputRun>) {
    if let Ok(mut j) = journal().lock() {
        j.push_runs(runs);
    }
}

#[cfg(any(test, windows))]
pub fn push_text_with_meta(text: &str, layout: LayoutTag, origin: RunOrigin) {
    if let Ok(mut j) = journal().lock() {
        j.push_text_internal(text, layout, origin);
    }
}

#[cfg(test)]
pub fn test_backspace() {
    if let Ok(mut j) = journal().lock() {
        j.backspace();
    }
}

#[cfg(test)]
pub fn runs_snapshot() -> Vec<InputRun> {
    journal()
        .lock()
        .ok()
        .map_or_else(Vec::new, |j| j.runs.iter().cloned().collect())
}

#[cfg(any(test, windows))]
pub fn invalidate() {
    if let Ok(mut j) = journal().lock() {
        j.clear();
    }
}

#[cfg(any(test, windows))]
pub fn last_char_triggers_autoconvert() -> bool {
    let Ok(j) = journal().lock() else {
        return false;
    };

    let Some(last) = j.last_char() else {
        return false;
    };

    if matches!(last, '.' | ',' | '!' | '?' | ';' | ':') {
        return j
            .prev_char_before_last()
            .is_some_and(|prev| !prev.is_whitespace());
    }

    if last.is_whitespace() {
        return j
            .prev_char_before_last()
            .is_some_and(|prev| !prev.is_whitespace());
    }

    false
}
