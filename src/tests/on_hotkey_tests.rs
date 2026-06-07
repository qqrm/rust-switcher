use windows::Win32::Foundation::WPARAM;

use crate::{
    input::hotkeys::{
        HK_CONVERT_LAST_SEQUENCE_ID, HK_CONVERT_LAST_WORD_ID, HK_CONVERT_SELECTION_ID,
        HK_PAUSE_TOGGLE_ID, HK_SMART_CONVERT_LAST_SEQUENCE_ID, HK_SMART_CONVERT_LAST_WORD_ID,
        HK_SMART_CONVERT_SELECTION_ID, HK_SWITCH_LAYOUT_ID, HotkeyAction, action_from_id,
    },
    platform::win::{hotkey_action_from_wparam, hotkey_id_from_wparam},
};

#[test]
fn hotkey_id_from_wparam_roundtrip() {
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_CONVERT_LAST_WORD_ID as usize)),
        HK_CONVERT_LAST_WORD_ID
    );
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_CONVERT_LAST_SEQUENCE_ID as usize)),
        HK_CONVERT_LAST_SEQUENCE_ID
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
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_SMART_CONVERT_LAST_WORD_ID as usize)),
        HK_SMART_CONVERT_LAST_WORD_ID
    );
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_SMART_CONVERT_LAST_SEQUENCE_ID as usize)),
        HK_SMART_CONVERT_LAST_SEQUENCE_ID
    );
    assert_eq!(
        hotkey_id_from_wparam(WPARAM(HK_SMART_CONVERT_SELECTION_ID as usize)),
        HK_SMART_CONVERT_SELECTION_ID
    );
}

#[test]
fn action_from_id_known_values() {
    assert_eq!(
        action_from_id(HK_CONVERT_LAST_WORD_ID),
        Some(HotkeyAction::ConvertLastWord)
    );
    assert_eq!(
        action_from_id(HK_CONVERT_LAST_SEQUENCE_ID),
        Some(HotkeyAction::ConvertLastSequence)
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
    assert_eq!(
        action_from_id(HK_SMART_CONVERT_LAST_WORD_ID),
        Some(HotkeyAction::SmartConvertLastWord)
    );
    assert_eq!(
        action_from_id(HK_SMART_CONVERT_LAST_SEQUENCE_ID),
        Some(HotkeyAction::SmartConvertLastSequence)
    );
    assert_eq!(
        action_from_id(HK_SMART_CONVERT_SELECTION_ID),
        Some(HotkeyAction::SmartConvertSelection)
    );
}

#[test]
fn action_from_id_unknown_is_none() {
    assert_eq!(action_from_id(0), None);
    assert_eq!(action_from_id(19999), None);
    assert_eq!(action_from_id(29999), None);
}

#[test]
fn hotkey_action_from_wparam_known_values() {
    assert_eq!(
        hotkey_action_from_wparam(WPARAM(HK_CONVERT_LAST_WORD_ID as usize)),
        Some(HotkeyAction::ConvertLastWord)
    );
    assert_eq!(
        hotkey_action_from_wparam(WPARAM(HK_CONVERT_LAST_SEQUENCE_ID as usize)),
        Some(HotkeyAction::ConvertLastSequence)
    );
    assert_eq!(
        hotkey_action_from_wparam(WPARAM(HK_PAUSE_TOGGLE_ID as usize)),
        Some(HotkeyAction::PauseToggle)
    );
    assert_eq!(
        hotkey_action_from_wparam(WPARAM(HK_CONVERT_SELECTION_ID as usize)),
        Some(HotkeyAction::ConvertSelection)
    );
    assert_eq!(
        hotkey_action_from_wparam(WPARAM(HK_SWITCH_LAYOUT_ID as usize)),
        Some(HotkeyAction::SwitchLayout)
    );
    assert_eq!(
        hotkey_action_from_wparam(WPARAM(HK_SMART_CONVERT_LAST_WORD_ID as usize)),
        Some(HotkeyAction::SmartConvertLastWord)
    );
    assert_eq!(
        hotkey_action_from_wparam(WPARAM(HK_SMART_CONVERT_LAST_SEQUENCE_ID as usize)),
        Some(HotkeyAction::SmartConvertLastSequence)
    );
    assert_eq!(
        hotkey_action_from_wparam(WPARAM(HK_SMART_CONVERT_SELECTION_ID as usize)),
        Some(HotkeyAction::SmartConvertSelection)
    );
}

#[test]
fn hotkey_action_from_wparam_unknown_is_none() {
    assert_eq!(hotkey_action_from_wparam(WPARAM(0)), None);
    assert_eq!(hotkey_action_from_wparam(WPARAM(19999)), None);
    assert_eq!(hotkey_action_from_wparam(WPARAM(29999)), None);
}
