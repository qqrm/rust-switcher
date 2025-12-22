use windows::Win32::Foundation::HWND;

use crate::{
    config,
    win::{
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
        return handle_keydown_capture(state, chord, is_mod, now_ms);
    }

    handle_keydown_runtime(hwnd, state, chord, is_mod, now_ms)
}

pub(crate) fn handle_keydown_capture(
    state: &mut crate::app::AppState,
    chord: config::HotkeyChord,
    is_mod: bool,
    now_ms: u64,
) -> windows::core::Result<HookDecision> {
    let Some(slot) = state.hotkey_capture.slot else {
        return Ok(HookDecision::Pass);
    };

    if is_mod {
        state.hotkey_capture.pending_mods = chord.mods;
        state.hotkey_capture.pending_mods_vks = chord.mods_vks;
        state.hotkey_capture.pending_mods_valid = true;
        state.hotkey_capture.saw_non_mod = false;
        return Ok(HookDecision::Swallow);
    }

    state.hotkey_capture.saw_non_mod = true;
    state.hotkey_capture.pending_mods_valid = false;

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
