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
        return handle_keyup_capture(hwnd, state, is_mod, now_ms);
    }

    handle_keyup_runtime(hwnd, state, is_mod, now_ms)
}

pub(crate) fn handle_keyup_capture(
    hwnd: HWND,
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
    crate::platform::win::touch_hotkey_settings_control(hwnd, state);

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
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT};

    use super::*;
    use crate::{
        app::{AppState, HotkeySequenceValues},
        config::{HotkeyChord, HotkeySequence, MODVK_LALT, MODVK_LSHIFT, MODVK_RCTRL},
        platform::win::keyboard::{
            keydown::handle_keydown_in_state,
            mods::{reset_mods_state, update_mods_down_press, update_mods_down_release},
        },
    };

    const VK_LALT: u32 = 0xA4;
    const VK_LSHIFT: u32 = 0xA0;
    const VK_RCTRL: u32 = 0xA3;

    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn double_left_alt_sequence() -> HotkeySequence {
        let chord = HotkeyChord {
            mods: MOD_ALT.0,
            mods_vks: MODVK_LALT,
            vk: None,
        };

        HotkeySequence {
            first: chord,
            second: Some(chord),
            third: None,
            max_gap_ms: 1000,
        }
    }

    fn modifier_sequence(mods: u32, mods_vks: u32, len: usize) -> HotkeySequence {
        let chord = HotkeyChord {
            mods,
            mods_vks,
            vk: None,
        };

        HotkeySequence {
            first: chord,
            second: (len >= 2).then_some(chord),
            third: (len >= 3).then_some(chord),
            max_gap_ms: 1000,
        }
    }

    fn tap_modifier(
        state: &mut AppState,
        vk: u32,
        down_ms: u64,
        up_ms: u64,
    ) -> (HookDecision, HookDecision) {
        update_mods_down_press(vk);
        let down = handle_keydown_in_state(HWND::default(), state, vk, true, down_ms)
            .expect("keydown should succeed");
        update_mods_down_release(vk);
        let up = handle_keyup_in_state(HWND::default(), state, vk, true, up_ms)
            .expect("keyup should succeed");
        (down, up)
    }

    #[test]
    fn modifier_only_last_sequence_advances_on_first_left_alt_release() {
        let _guard = test_lock();
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
        let _guard = test_lock();
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

    #[test]
    fn modifier_only_triple_shift_defers_double_shift_runtime_action() {
        let _guard = test_lock();
        reset_mods_state();

        let mut state = AppState {
            active_hotkey_sequences: HotkeySequenceValues {
                last_word: Some(modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT, 2)),
                smart_last_word: Some(modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT, 3)),
                ..Default::default()
            },
            ..Default::default()
        };

        let (_, first_up) = tap_modifier(&mut state, VK_LSHIFT, 100, 150);
        let (_, second_up) = tap_modifier(&mut state, VK_LSHIFT, 250, 300);
        let (_, third_up) = tap_modifier(&mut state, VK_LSHIFT, 400, 450);

        assert_eq!(first_up, HookDecision::Swallow);
        assert_eq!(second_up, HookDecision::Swallow);
        assert_eq!(third_up, HookDecision::Swallow);
        assert!(!state.hotkey_sequence_progress.last_word.waiting_second);
        assert!(!state.hotkey_sequence_progress.smart_last_word.waiting_second);

        reset_mods_state();
    }

    #[test]
    fn modifier_only_triple_alt_defers_double_alt_runtime_action() {
        let _guard = test_lock();
        reset_mods_state();

        let mut state = AppState {
            active_hotkey_sequences: HotkeySequenceValues {
                last_sequence: Some(modifier_sequence(MOD_ALT.0, MODVK_LALT, 2)),
                smart_last_sequence: Some(modifier_sequence(MOD_ALT.0, MODVK_LALT, 3)),
                ..Default::default()
            },
            ..Default::default()
        };

        let (_, first_up) = tap_modifier(&mut state, VK_LALT, 100, 150);
        let (_, second_up) = tap_modifier(&mut state, VK_LALT, 250, 300);
        let (_, third_up) = tap_modifier(&mut state, VK_LALT, 400, 450);

        assert_eq!(first_up, HookDecision::Swallow);
        assert_eq!(second_up, HookDecision::Swallow);
        assert_eq!(third_up, HookDecision::Swallow);
        assert!(!state.hotkey_sequence_progress.last_sequence.waiting_second);
        assert!(!state.hotkey_sequence_progress.smart_last_sequence.waiting_second);

        reset_mods_state();
    }

    #[test]
    fn modifier_only_runtime_hotkeys_do_not_cross_trigger_between_slots() {
        let _guard = test_lock();
        reset_mods_state();

        let mut state = AppState {
            active_hotkey_sequences: HotkeySequenceValues {
                last_word: Some(modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT, 2)),
                last_sequence: Some(modifier_sequence(MOD_ALT.0, MODVK_LALT, 2)),
                pause: Some(modifier_sequence(MOD_CONTROL.0, MODVK_RCTRL, 2)),
                smart_last_word: Some(modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT, 3)),
                smart_last_sequence: Some(modifier_sequence(MOD_ALT.0, MODVK_LALT, 3)),
                smart_selection: Some(modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT, 3)),
                ..Default::default()
            },
            ..Default::default()
        };

        let (_, shift_1) = tap_modifier(&mut state, VK_LSHIFT, 100, 150);
        let (_, shift_2) = tap_modifier(&mut state, VK_LSHIFT, 250, 300);
        assert_eq!(shift_1, HookDecision::Swallow);
        assert_eq!(shift_2, HookDecision::Swallow);
        assert!(state.hotkey_sequence_progress.smart_last_word.waiting_second);
        assert!(!state.hotkey_sequence_progress.last_word.waiting_second);

        let (_, alt_1) = tap_modifier(&mut state, VK_LALT, 1_500, 1_550);
        let (_, alt_2) = tap_modifier(&mut state, VK_LALT, 1_650, 1_700);
        assert_eq!(alt_1, HookDecision::Swallow);
        assert_eq!(alt_2, HookDecision::Swallow);
        assert!(state.hotkey_sequence_progress.smart_last_sequence.waiting_second);
        assert!(!state.hotkey_sequence_progress.last_sequence.waiting_second);
        assert!(!state.hotkey_sequence_progress.smart_last_word.waiting_second);

        let (_, ctrl_1) = tap_modifier(&mut state, VK_RCTRL, 3_000, 3_050);
        let (_, ctrl_2) = tap_modifier(&mut state, VK_RCTRL, 3_150, 3_200);
        assert_eq!(ctrl_1, HookDecision::Swallow);
        assert_eq!(ctrl_2, HookDecision::Swallow);
        assert!(!state.hotkey_sequence_progress.pause.waiting_second);
        assert!(!state.hotkey_sequence_progress.last_word.waiting_second);
        assert!(!state.hotkey_sequence_progress.last_sequence.waiting_second);

        reset_mods_state();
    }
}
