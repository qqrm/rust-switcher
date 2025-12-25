use tracing_test::traced_test;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::WM_HOTKEY,
};

use crate::{
    hotkeys::{
        HK_CONVERT_LAST_WORD_ID, HK_CONVERT_SELECTION_ID, HK_PAUSE_TOGGLE_ID, HK_SWITCH_LAYOUT_ID,
        HotkeyAction, action_from_id,
    },
    win::{hotkey_id_from_wparam, wndproc},
};

fn dummy_hwnd() -> HWND {
    HWND(12345 as *mut core::ffi::c_void)
}

fn call_hotkey(id: i32) -> LRESULT {
    wndproc(dummy_hwnd(), WM_HOTKEY, WPARAM(id as usize), LPARAM(0))
}

#[test]
fn hotkey_id_from_wparam_roundtrip() {
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_CONVERT_LAST_WORD_ID as usize)),
        HK_CONVERT_LAST_WORD_ID
    );
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_PAUSE_TOGGLE_ID as usize)),
        HK_PAUSE_TOGGLE_ID
    );
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_CONVERT_SELECTION_ID as usize)),
        HK_CONVERT_SELECTION_ID
    );
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_SWITCH_LAYOUT_ID as usize)),
        HK_SWITCH_LAYOUT_ID
    );
}

#[test]
fn action_from_id_known_values() {
    assert_eq!(
        action_from_id(HK_CONVERT_LAST_WORD_ID),
        Some(HotkeyAction::ConvertLastWord)
    );
    assert_eq!(
        action_from_id(HK_PAUSE_TOGGLE_ID),
        Some(HotkeyAction::PauseToggle)
    );
    assert_eq!(
        action_from_id(HK_CONVERT_SELECTION_ID),
        Some(HotkeyAction::ConvertSelection)
    );
    assert_eq!(
        action_from_id(HK_SWITCH_LAYOUT_ID),
        Some(HotkeyAction::SwitchLayout)
    );
}

#[test]
fn action_from_id_unknown_is_none() {
    assert_eq!(action_from_id(0), None);
    assert_eq!(action_from_id(19999), None);
    assert_eq!(action_from_id(29999), None);
}

#[test]
fn wndproc_hotkey_unknown_id_returns_zero() {
    let r = call_hotkey(29999);
    assert_eq!(r, LRESULT(0));
}

#[test]
fn wndproc_hotkey_pause_toggle_returns_zero() {
    let r = call_hotkey(HK_PAUSE_TOGGLE_ID);
    assert_eq!(r, LRESULT(0));
}

#[test]
fn wndproc_hotkey_switch_layout_returns_zero() {
    let r = call_hotkey(HK_SWITCH_LAYOUT_ID);
    assert_eq!(r, LRESULT(0));
}

#[traced_test]
#[test]
fn wndproc_hotkey_convert_selection_emits_trace() {
    let r = call_hotkey(HK_CONVERT_SELECTION_ID);
    assert_eq!(r, LRESULT(0));
    assert!(logs_contain("convert_selection called"));
}

#[test]
fn wndproc_hotkey_convert_last_word_does_not_panic() {
    let r = call_hotkey(HK_CONVERT_LAST_WORD_ID);
    assert_eq!(r, LRESULT(0));
}
