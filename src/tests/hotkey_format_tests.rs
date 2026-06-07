#![cfg(windows)]

use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

use crate::{
    config::{self, MODVK_LSHIFT},
    platform::win::hotkey_format::{format_hotkey, format_hotkey_sequence},
};

#[test]
fn format_hotkey_none() {
    assert_eq!(format_hotkey(None), "None");
}

#[test]
fn format_hotkey_letter_fast_path() {
    let hk = config::Hotkey {
        mods: MOD_CONTROL.0,
        vk: u32::from(b'A'),
    };
    let s = format_hotkey(Some(hk));
    assert!(s.contains("Ctrl"));
    assert!(s.contains("A"));
}

#[test]
fn format_hotkey_multiple_mods_fast_path() {
    let hk = config::Hotkey {
        mods: MOD_CONTROL.0 | MOD_SHIFT.0 | MOD_ALT.0 | MOD_WIN.0,
        vk: u32::from(b'9'),
    };
    let s = format_hotkey(Some(hk));
    assert!(s.contains("Ctrl"));
    assert!(s.contains("Shift"));
    assert!(s.contains("Alt"));
    assert!(s.contains("Win"));
    assert!(s.contains("9"));
}

#[test]
fn format_hotkey_sequence_includes_third_chord() {
    let chord = config::HotkeyChord {
        mods: MOD_SHIFT.0,
        mods_vks: MODVK_LSHIFT,
        vk: None,
    };
    let seq = config::HotkeySequence {
        first: chord,
        second: Some(chord),
        third: Some(chord),
        max_gap_ms: 1000,
    };

    assert_eq!(format_hotkey_sequence(Some(seq)), "LShift; LShift; LShift");
}
