use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::SystemInformation::GetTickCount64,
    UI::{
        Input::KeyboardAndMouse::{
            MAPVK_VSC_TO_VK_EX, MapVirtualKeyW, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_MENU,
            VK_RCONTROL, VK_RMENU, VK_SHIFT,
        },
        WindowsAndMessaging::{
            CallNextHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, PostMessageW,
            SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_HOTKEY, WM_KEYDOWN,
            WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};

use crate::{config, helpers};

static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static MODS_DOWN: AtomicU32 = AtomicU32::new(0);
static MODVKS_DOWN: AtomicU32 = AtomicU32::new(0);

fn now_tick_ms() -> u64 {
    unsafe { GetTickCount64() }
}

fn mod_bit_for_vk(vk: u32) -> Option<u32> {
    match vk {
        0xA2 | 0xA3 => Some(windows::Win32::UI::Input::KeyboardAndMouse::MOD_CONTROL.0), // VK_LCONTROL VK_RCONTROL
        0xA0 | 0xA1 => Some(windows::Win32::UI::Input::KeyboardAndMouse::MOD_SHIFT.0), // VK_LSHIFT VK_RSHIFT
        0xA4 | 0xA5 => Some(windows::Win32::UI::Input::KeyboardAndMouse::MOD_ALT.0), // VK_LMENU VK_RMENU
        0x5B | 0x5C => Some(windows::Win32::UI::Input::KeyboardAndMouse::MOD_WIN.0), // VK_LWIN VK_RWIN
        _ => None,
    }
}

fn mod_vk_bit_for_vk(vk: u32) -> Option<u32> {
    match vk {
        0xA2 => Some(config::MODVK_LCTRL),  // VK_LCONTROL
        0xA3 => Some(config::MODVK_RCTRL),  // VK_RCONTROL
        0xA0 => Some(config::MODVK_LSHIFT), // VK_LSHIFT
        0xA1 => Some(config::MODVK_RSHIFT), // VK_RSHIFT
        0xA4 => Some(config::MODVK_LALT),   // VK_LMENU
        0xA5 => Some(config::MODVK_RALT),   // VK_RMENU
        0x5B => Some(config::MODVK_LWIN),   // VK_LWIN
        0x5C => Some(config::MODVK_RWIN),   // VK_RWIN
        _ => None,
    }
}

fn is_keydown_msg(msg: u32) -> bool {
    msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN
}

fn normalize_vk(kb: &KBDLLHOOKSTRUCT) -> u32 {
    let vk = kb.vkCode;
    let extended = kb.flags.contains(LLKHF_EXTENDED);

    match vk {
        x if x == VK_SHIFT.0 as u32 => {
            // Reliable left right shift resolution based on scan code mapping.
            let mapped = unsafe { MapVirtualKeyW(kb.scanCode, MAPVK_VSC_TO_VK_EX) };
            if mapped != 0 { mapped } else { vk }
        }
        x if x == VK_CONTROL.0 as u32 => {
            if extended {
                VK_RCONTROL.0 as u32
            } else {
                VK_LCONTROL.0 as u32
            }
        }
        x if x == VK_MENU.0 as u32 => {
            if extended {
                VK_RMENU.0 as u32
            } else {
                VK_LMENU.0 as u32
            }
        }
        _ => vk,
    }
}

fn is_keyup_msg(msg: u32) -> bool {
    msg == WM_KEYUP || msg == WM_SYSKEYUP
}

fn chord_to_hotkey(ch: config::HotkeyChord) -> config::Hotkey {
    config::Hotkey {
        vk: ch.vk.unwrap_or(0),
        mods: ch.mods,
    }
}

fn chord_matches(template: config::HotkeyChord, input: config::HotkeyChord) -> bool {
    if template.mods != input.mods {
        return false;
    }
    if template.vk != input.vk {
        return false;
    }
    if template.mods_vks == 0 {
        return true;
    }
    template.mods_vks == input.mods_vks
}

fn main_hwnd() -> Option<HWND> {
    let raw = MAIN_HWND.load(Ordering::Relaxed);
    if raw == 0 {
        None
    } else {
        Some(HWND(raw as *mut _))
    }
}

#[allow(dead_code)]
fn should_swallow(hwnd: HWND) -> bool {
    super::with_state_mut(hwnd, |s| s.hotkey_capture.active).unwrap_or(false)
}

fn push_chord_capture(
    existing: Option<config::HotkeySequence>,
    chord: config::HotkeyChord,
    now_ms: u64,
    last_input_tick_ms: &mut u64,
) -> config::HotkeySequence {
    const DEFAULT_GAP_MS: u32 = 1000;
    const RESET_AFTER_MS: u64 = 2000;

    let existing = match (*last_input_tick_ms, existing) {
        (0, s) => s,
        (prev, s) if now_ms.saturating_sub(prev) > RESET_AFTER_MS => None,
        (_, s) => s,
    };

    let seq = match existing {
        None => config::HotkeySequence {
            first: chord,
            second: None,
            max_gap_ms: DEFAULT_GAP_MS,
        },
        Some(mut s) => match s.second {
            None => {
                s.second = Some(chord);
                s
            }
            Some(prev_second) => {
                s.first = prev_second;
                s.second = Some(chord);
                s
            }
        },
    };

    *last_input_tick_ms = now_ms;
    seq
}

fn progress_for_slot_mut(
    state: &mut crate::app::AppState,
    slot: crate::app::HotkeySlot,
) -> &mut crate::app::SequenceProgress {
    match slot {
        crate::app::HotkeySlot::LastWord => &mut state.hotkey_sequence_progress.last_word,
        crate::app::HotkeySlot::Pause => &mut state.hotkey_sequence_progress.pause,
        crate::app::HotkeySlot::Selection => &mut state.hotkey_sequence_progress.selection,
        crate::app::HotkeySlot::SwitchLayout => &mut state.hotkey_sequence_progress.switch_layout,
    }
}

fn hotkey_id_for_slot(slot: crate::app::HotkeySlot) -> i32 {
    match slot {
        crate::app::HotkeySlot::LastWord => crate::hotkeys::HK_CONVERT_LAST_WORD_ID,
        crate::app::HotkeySlot::Pause => crate::hotkeys::HK_PAUSE_TOGGLE_ID,
        crate::app::HotkeySlot::Selection => crate::hotkeys::HK_CONVERT_SELECTION_ID,
        crate::app::HotkeySlot::SwitchLayout => crate::hotkeys::HK_SWITCH_LAYOUT_ID,
    }
}

fn post_hotkey(hwnd: HWND, id: i32) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_HOTKEY, WPARAM(id as usize), LPARAM(0));
    }
}

fn effective_gap_ms(slot: crate::app::HotkeySlot, seq: config::HotkeySequence) -> u64 {
    match slot {
        crate::app::HotkeySlot::SwitchLayout => 1000,
        _ => seq.max_gap_ms as u64,
    }
}

fn try_match_sequence(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    slot: crate::app::HotkeySlot,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> bool {
    let Some(seq) = state.active_hotkey_sequences.get(slot) else {
        return false;
    };

    // Single chord sequence
    if seq.second.is_none() {
        if chord_matches(seq.first, chord) {
            post_hotkey(hwnd, hotkey_id_for_slot(slot));
            return true;
        }
        return false;
    }

    let second = seq.second.unwrap();
    let gap_ms = effective_gap_ms(slot, seq);

    let prog = progress_for_slot_mut(state, slot);

    if prog.waiting_second && now_ms.saturating_sub(prog.first_tick_ms) > gap_ms {
        prog.waiting_second = false;
        prog.first_tick_ms = 0;
    }

    if prog.waiting_second {
        if chord_matches(second, chord) {
            prog.waiting_second = false;
            prog.first_tick_ms = 0;

            post_hotkey(hwnd, hotkey_id_for_slot(slot));
            return true;
        }

        // If user repeats first chord, keep waiting window alive
        if chord_matches(seq.first, chord) {
            prog.first_tick_ms = now_ms;
            return true;
        }

        // Wrong second chord, reset to waiting first
        prog.waiting_second = false;
        prog.first_tick_ms = 0;
        return false;
    }

    if chord_matches(seq.first, chord) {
        prog.waiting_second = true;
        prog.first_tick_ms = now_ms;
        return true;
    }

    false
}

fn try_match_any_sequence(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> bool {
    for slot in [
        crate::app::HotkeySlot::SwitchLayout,
        crate::app::HotkeySlot::LastWord,
        crate::app::HotkeySlot::Selection,
        crate::app::HotkeySlot::Pause,
    ] {
        if try_match_sequence(hwnd, state, slot, chord, now_ms) {
            return true;
        }
    }
    false
}

fn handle_keydown(vk: u32, is_mod: bool) -> bool {
    if let Some(bit) = mod_bit_for_vk(vk) {
        MODS_DOWN.fetch_or(bit, Ordering::Relaxed);
    }
    if let Some(bit) = mod_vk_bit_for_vk(vk) {
        MODVKS_DOWN.fetch_or(bit, Ordering::Relaxed);
    }

    let Some(hwnd) = main_hwnd() else {
        return false;
    };

    let now_ms = now_tick_ms();

    super::with_state_mut(hwnd, |state| {
        if state.hotkey_capture.active {
            let Some(slot) = state.hotkey_capture.slot else {
                return false;
            };

            let mods = MODS_DOWN.load(Ordering::Relaxed);
            let mods_vks = MODVKS_DOWN.load(Ordering::Relaxed);

            if is_mod {
                state.hotkey_capture.pending_mods = mods;
                state.hotkey_capture.pending_mods_vks = mods_vks;
                state.hotkey_capture.pending_mods_valid = true;
                state.hotkey_capture.saw_non_mod = false;
                return true;
            }

            state.hotkey_capture.saw_non_mod = true;
            state.hotkey_capture.pending_mods_valid = false;

            let chord = config::HotkeyChord {
                mods,
                mods_vks,
                vk: Some(vk),
            };

            let prev = state.hotkey_sequence_values.get(slot);
            let seq = push_chord_capture(
                prev,
                chord,
                now_ms,
                &mut state.hotkey_capture.last_input_tick_ms,
            );

            state.hotkey_sequence_values.set(slot, Some(seq));
            state.hotkey_values.set(slot, Some(chord_to_hotkey(chord)));

            let text = super::format_hotkey_sequence(Some(seq));
            let target = match slot {
                crate::app::HotkeySlot::LastWord => state.hotkeys.last_word,
                crate::app::HotkeySlot::Pause => state.hotkeys.pause,
                crate::app::HotkeySlot::Selection => state.hotkeys.selection,
                crate::app::HotkeySlot::SwitchLayout => state.hotkeys.switch_layout,
            };

            let _ = helpers::set_edit_text(target, &text);
            return true;
        }

        let mods = MODS_DOWN.load(Ordering::Relaxed);
        let mods_vks = MODVKS_DOWN.load(Ordering::Relaxed);

        if is_mod {
            state.runtime_chord_capture.pending_mods = mods;
            state.runtime_chord_capture.pending_mods_vks = mods_vks;
            state.runtime_chord_capture.pending_mods_valid = true;
            state.runtime_chord_capture.saw_non_mod = false;
            return false;
        }

        state.runtime_chord_capture.saw_non_mod = true;
        state.runtime_chord_capture.pending_mods_valid = false;

        let chord = config::HotkeyChord {
            mods,
            mods_vks,
            vk: Some(vk),
        };

        try_match_any_sequence(hwnd, state, chord, now_ms)
    })
    .unwrap_or(false)
}

fn handle_keyup(vk: u32, is_mod: bool) -> bool {
    if let Some(bit) = mod_bit_for_vk(vk) {
        MODS_DOWN.fetch_and(!bit, Ordering::Relaxed);
    }
    if let Some(bit) = mod_vk_bit_for_vk(vk) {
        MODVKS_DOWN.fetch_and(!bit, Ordering::Relaxed);
    }

    let Some(hwnd) = main_hwnd() else {
        return false;
    };

    let now_ms = now_tick_ms();

    super::with_state_mut(hwnd, |state| {
        if state.hotkey_capture.active {
            let Some(slot) = state.hotkey_capture.slot else {
                return false;
            };

            if !is_mod {
                return true;
            }

            let mods_now = MODS_DOWN.load(Ordering::Relaxed);
            if !state.hotkey_capture.pending_mods_valid {
                return true;
            }
            if state.hotkey_capture.saw_non_mod {
                return true;
            }
            if mods_now != 0 {
                return true;
            }

            let chord = config::HotkeyChord {
                mods: state.hotkey_capture.pending_mods,
                mods_vks: state.hotkey_capture.pending_mods_vks,
                vk: None,
            };

            let prev = state.hotkey_sequence_values.get(slot);
            let seq = push_chord_capture(
                prev,
                chord,
                now_ms,
                &mut state.hotkey_capture.last_input_tick_ms,
            );

            state.hotkey_sequence_values.set(slot, Some(seq));
            state.hotkey_values.set(slot, Some(chord_to_hotkey(chord)));

            state.hotkey_capture.pending_mods_valid = false;
            state.hotkey_capture.pending_mods = 0;
            state.hotkey_capture.pending_mods_vks = 0;

            let text = super::format_hotkey_sequence(Some(seq));
            let target = match slot {
                crate::app::HotkeySlot::LastWord => state.hotkeys.last_word,
                crate::app::HotkeySlot::Pause => state.hotkeys.pause,
                crate::app::HotkeySlot::Selection => state.hotkeys.selection,
                crate::app::HotkeySlot::SwitchLayout => state.hotkeys.switch_layout,
            };

            let _ = helpers::set_edit_text(target, &text);
            return true;
        }

        if !is_mod {
            return false;
        }

        let mods_now = MODS_DOWN.load(Ordering::Relaxed);

        if !state.runtime_chord_capture.pending_mods_valid {
            return false;
        }
        if state.runtime_chord_capture.saw_non_mod {
            return false;
        }
        if mods_now != 0 {
            return false;
        }

        let chord = config::HotkeyChord {
            mods: state.runtime_chord_capture.pending_mods,
            mods_vks: state.runtime_chord_capture.pending_mods_vks,
            vk: None,
        };

        state.runtime_chord_capture = crate::app::RuntimeChordCapture::default();

        try_match_any_sequence(hwnd, state, chord, now_ms)
    })
    .unwrap_or(false)
}

extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        let h = HOOK_HANDLE.load(Ordering::Relaxed);
        let hook = if h == 0 {
            None
        } else {
            Some(HHOOK(h as *mut _))
        };
        return unsafe { CallNextHookEx(hook, code, wparam, lparam) };
    }

    let msg = wparam.0 as u32;
    let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let vk = normalize_vk(kb);
    let is_mod = mod_bit_for_vk(vk).is_some();

    if is_keydown_msg(msg) {
        if handle_keydown(vk, is_mod) {
            return LRESULT(1);
        }
    } else if is_keyup_msg(msg) && handle_keyup(vk, is_mod) {
        return LRESULT(1);
    }

    let h = HOOK_HANDLE.load(Ordering::Relaxed);
    let hook = if h == 0 {
        None
    } else {
        Some(HHOOK(h as *mut _))
    };
    unsafe { CallNextHookEx(hook, code, wparam, lparam) }
}

pub fn install(hwnd: HWND) {
    MAIN_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

    if HOOK_HANDLE.load(Ordering::Relaxed) != 0 {
        return;
    }

    match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), None, 0) } {
        Ok(h) => {
            HOOK_HANDLE.store(h.0 as isize, Ordering::Relaxed);
            #[cfg(debug_assertions)]
            eprintln!("RustSwitcher: WH_KEYBOARD_LL installed");
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("RustSwitcher: SetWindowsHookExW failed: {}", e);
        }
    }
}

pub fn uninstall() {
    let h = HOOK_HANDLE.swap(0, Ordering::Relaxed);
    if h == 0 {
        return;
    }

    unsafe {
        let _ = UnhookWindowsHookEx(HHOOK(h as *mut _));
    }

    #[cfg(debug_assertions)]
    eprintln!("RustSwitcher: WH_KEYBOARD_LL removed");
}
