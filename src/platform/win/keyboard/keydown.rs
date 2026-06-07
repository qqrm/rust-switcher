use windows::Win32::Foundation::HWND;

use crate::{
    config,
    platform::win::{
        keyboard::{
            HookDecision,
            capture::{push_chord_capture, store_captured_hotkey},
            main_hwnd,
            mods::{chord_from_vk, update_mods_down_press},
            now_tick_ms,
            sequence::try_match_any_sequence,
        },
        with_state_mut,
    },
};

pub(crate) fn handle_keydown(vk: u32, is_mod: bool) -> windows::core::Result<HookDecision> {
    update_mods_down_press(vk);

    let Some(hwnd) = main_hwnd() else {
        return Ok(HookDecision::Pass);
    };

    let now_ms = now_tick_ms();

    with_state_mut(hwnd, |state| {
        handle_keydown_in_state(hwnd, state, vk, is_mod, now_ms)
    })
    .unwrap_or(Ok(HookDecision::Pass))
}

pub(crate) fn handle_keydown_in_state(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    vk: u32,
    is_mod: bool,
    now_ms: u64,
) -> windows::core::Result<HookDecision> {
    let chord = chord_from_vk(vk);

    if state.hotkey_capture.active {
        return handle_keydown_capture(hwnd, state, vk, chord, is_mod, now_ms);
    }

    handle_keydown_runtime(hwnd, state, chord, is_mod, now_ms)
}

pub(crate) fn handle_keydown_capture(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    vk: u32,
    chord: config::HotkeyChord,
    is_mod: bool,
    now_ms: u64,
) -> windows::core::Result<HookDecision> {
    if matches!(vk, 0x0D | 0x1B) {
        crate::platform::win::stop_hotkey_capture_ui(hwnd, state);
        return Ok(HookDecision::Swallow);
    }

    let Some(slot) = state.hotkey_capture.slot else {
        return Ok(HookDecision::Pass);
    };

    if is_mod {
        crate::platform::win::touch_hotkey_settings_control(hwnd, state);
        state.hotkey_capture.pending_mods = chord.mods;
        state.hotkey_capture.pending_mods_vks = chord.mods_vks;
        state.hotkey_capture.pending_mods_valid = true;
        state.hotkey_capture.saw_non_mod = false;
        return Ok(HookDecision::Swallow);
    }

    state.hotkey_capture.saw_non_mod = true;
    state.hotkey_capture.pending_mods_valid = false;
    crate::platform::win::touch_hotkey_settings_control(hwnd, state);

    let prev = state.hotkey_sequence_values.get(slot);
    let seq = push_chord_capture(
        prev,
        chord,
        now_ms,
        &mut state.hotkey_capture.last_input_tick_ms,
    );

    store_captured_hotkey(state, slot, chord, seq)?;
    Ok(HookDecision::Swallow)
}

pub(crate) fn handle_keydown_runtime(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    chord: config::HotkeyChord,
    is_mod: bool,
    now_ms: u64,
) -> windows::core::Result<HookDecision> {
    if is_mod {
        state.runtime_chord_capture.pending_mods = chord.mods;
        state.runtime_chord_capture.pending_mods_vks = chord.mods_vks;
        state.runtime_chord_capture.pending_mods_valid = true;
        state.runtime_chord_capture.saw_non_mod = false;
        return Ok(HookDecision::Pass);
    }

    state.runtime_chord_capture.saw_non_mod = true;
    state.runtime_chord_capture.pending_mods_valid = false;

    let matched = try_match_any_sequence(hwnd, state, chord, now_ms)?;
    Ok(if matched {
        HookDecision::Swallow
    } else {
        HookDecision::Pass
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::HotkeySlot, config};

    fn chord() -> config::HotkeyChord {
        config::HotkeyChord {
            mods: 0,
            mods_vks: 0,
            vk: Some(u32::from(b'A')),
        }
    }

    #[test]
    fn enter_stops_hotkey_capture() {
        let mut state = crate::app::AppState::default();
        state.hotkey_capture.start(HotkeySlot::LastWord);

        let decision =
            handle_keydown_capture(HWND::default(), &mut state, 0x0D, chord(), false, 100)
                .expect("capture should succeed");

        assert_eq!(decision, HookDecision::Swallow);
        assert!(!state.hotkey_capture.active);
        assert_eq!(state.hotkey_capture.slot, None);
    }

    #[test]
    fn escape_stops_hotkey_capture() {
        let mut state = crate::app::AppState::default();
        state.hotkey_capture.start(HotkeySlot::LastSequence);

        let decision =
            handle_keydown_capture(HWND::default(), &mut state, 0x1B, chord(), false, 100)
                .expect("capture should succeed");

        assert_eq!(decision, HookDecision::Swallow);
        assert!(!state.hotkey_capture.active);
        assert_eq!(state.hotkey_capture.slot, None);
    }
}
