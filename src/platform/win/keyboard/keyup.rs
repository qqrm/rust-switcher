use windows::Win32::Foundation::HWND;

use crate::{
    config,
    platform::win::{
        keyboard::{
            HookDecision,
            capture::{push_chord_capture, store_captured_hotkey},
            main_hwnd,
            mods::{mods_now, update_mods_down_release},
            now_tick_ms,
            sequence::try_match_any_sequence,
        },
        with_state_mut,
    },
};

pub(crate) fn handle_keyup(vk: u32, is_mod: bool) -> windows::core::Result<HookDecision> {
    update_mods_down_release(vk);

    let Some(hwnd) = main_hwnd() else {
        return Ok(HookDecision::Pass);
    };

    let now_ms = now_tick_ms();

    with_state_mut(hwnd, |state| {
        handle_keyup_in_state(hwnd, state, vk, is_mod, now_ms)
    })
    .unwrap_or(Ok(HookDecision::Pass))
}

pub(crate) fn handle_keyup_in_state(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    _vk: u32,
    is_mod: bool,
    now_ms: u64,
) -> windows::core::Result<HookDecision> {
    if state.hotkey_capture.active {
        return handle_keyup_capture(state, is_mod, now_ms);
    }

    handle_keyup_runtime(hwnd, state, is_mod, now_ms)
}

pub(crate) fn handle_keyup_capture(
    state: &mut crate::app::AppState,
    is_mod: bool,
    now_ms: u64,
) -> windows::core::Result<HookDecision> {
    let Some(slot) = state.hotkey_capture.slot else {
        return Ok(HookDecision::Pass);
    };

    if !is_mod {
        return Ok(HookDecision::Swallow);
    }

    if !state.hotkey_capture.pending_mods_valid {
        return Ok(HookDecision::Swallow);
    }
    if state.hotkey_capture.saw_non_mod {
        return Ok(HookDecision::Swallow);
    }

    let mods_now = mods_now();
    if mods_now != 0 {
        return Ok(HookDecision::Swallow);
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

    state.hotkey_capture.pending_mods_valid = false;
    state.hotkey_capture.pending_mods = 0;
    state.hotkey_capture.pending_mods_vks = 0;

    store_captured_hotkey(state, slot, chord, seq)?;
    Ok(HookDecision::Swallow)
}

pub(crate) fn handle_keyup_runtime(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    is_mod: bool,
    now_ms: u64,
) -> windows::core::Result<HookDecision> {
    if !is_mod {
        return Ok(HookDecision::Pass);
    }

    if !state.runtime_chord_capture.pending_mods_valid {
        return Ok(HookDecision::Pass);
    }
    if state.runtime_chord_capture.saw_non_mod {
        return Ok(HookDecision::Pass);
    }

    let mods_now = mods_now();
    if mods_now != 0 {
        return Ok(HookDecision::Pass);
    }

    let chord = config::HotkeyChord {
        mods: state.runtime_chord_capture.pending_mods,
        mods_vks: state.runtime_chord_capture.pending_mods_vks,
        vk: None,
    };

    state.runtime_chord_capture = crate::app::RuntimeChordCapture::default();

    let matched = try_match_any_sequence(hwnd, state, chord, now_ms)?;
    Ok(if matched {
        HookDecision::Swallow
    } else {
        HookDecision::Pass
    })
}

#[cfg(test)]
mod tests {
    use windows::Win32::UI::Input::KeyboardAndMouse::MOD_ALT;

    use super::*;
    use crate::{
        app::{AppState, HotkeySequenceValues},
        config::{HotkeyChord, HotkeySequence, MODVK_LALT},
        platform::win::keyboard::{
            keydown::handle_keydown_in_state,
            mods::{reset_mods_state, update_mods_down_press, update_mods_down_release},
        },
    };

    const VK_LALT: u32 = 0xA4;

    fn double_left_alt_sequence() -> HotkeySequence {
        let chord = HotkeyChord {
            mods: MOD_ALT.0,
            mods_vks: MODVK_LALT,
            vk: None,
        };

        HotkeySequence {
            first: chord,
            second: Some(chord),
            max_gap_ms: 1000,
        }
    }

    #[test]
    fn modifier_only_last_sequence_advances_on_first_left_alt_release() {
        reset_mods_state();

        let mut state = AppState {
            active_hotkey_sequences: HotkeySequenceValues {
                last_sequence: Some(double_left_alt_sequence()),
                ..Default::default()
            },
            ..Default::default()
        };

        update_mods_down_press(VK_LALT);
        let down = handle_keydown_in_state(HWND::default(), &mut state, VK_LALT, true, 100)
            .expect("keydown should succeed");
        update_mods_down_release(VK_LALT);
        let up = handle_keyup_in_state(HWND::default(), &mut state, VK_LALT, true, 150)
            .expect("keyup should succeed");

        assert_eq!(down, HookDecision::Pass);
        assert_eq!(up, HookDecision::Swallow);
        assert!(state.hotkey_sequence_progress.last_sequence.waiting_second);

        reset_mods_state();
    }

    #[test]
    fn modifier_only_last_sequence_triggers_on_second_left_alt_release() {
        reset_mods_state();

        let mut state = AppState {
            active_hotkey_sequences: HotkeySequenceValues {
                last_sequence: Some(double_left_alt_sequence()),
                ..Default::default()
            },
            ..Default::default()
        };

        for (down_ms, up_ms) in [(100_u64, 150_u64), (250_u64, 300_u64)] {
            update_mods_down_press(VK_LALT);
            let down = handle_keydown_in_state(HWND::default(), &mut state, VK_LALT, true, down_ms)
                .expect("keydown should succeed");
            update_mods_down_release(VK_LALT);
            let up = handle_keyup_in_state(HWND::default(), &mut state, VK_LALT, true, up_ms)
                .expect("keyup should succeed");

            assert_eq!(down, HookDecision::Pass);
            assert_eq!(up, HookDecision::Swallow);
        }

        assert!(!state.hotkey_sequence_progress.last_sequence.waiting_second);

        reset_mods_state();
    }
}
