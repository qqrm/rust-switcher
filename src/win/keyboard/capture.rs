use windows::Win32::Foundation::HWND;

use crate::{config, helpers, win::format_hotkey_sequence};

pub(crate) fn chord_to_hotkey(ch: config::HotkeyChord) -> config::Hotkey {
    config::Hotkey {
        vk: ch.vk.unwrap_or(0),
        mods: ch.mods,
    }
}

pub(crate) fn push_chord_capture(
    existing: Option<config::HotkeySequence>,
    chord: config::HotkeyChord,
    now_ms: u64,
    last_input_tick_ms: &mut u64,
) -> config::HotkeySequence {
    const DEFAULT_GAP_MS: u32 = 1000;
    const RESET_AFTER_MS: u64 = 2000;

    let existing = match (*last_input_tick_ms, existing) {
        (0, _) => None,
        (prev, _) if now_ms.saturating_sub(prev) > RESET_AFTER_MS => None,
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

pub(crate) fn store_captured_hotkey(
    state: &mut crate::app::AppState,
    slot: crate::app::HotkeySlot,
    chord: config::HotkeyChord,
    seq: config::HotkeySequence,
) -> windows::core::Result<()> {
    state.hotkey_sequence_values.set(slot, Some(seq));
    state.hotkey_values.set(slot, Some(chord_to_hotkey(chord)));

    let text = format_hotkey_sequence(Some(seq));
    let target = ui_hotkey_target(state, slot);

    helpers::set_edit_text(target, &text)?;
    Ok(())
}

pub(crate) fn ui_hotkey_target(state: &crate::app::AppState, slot: crate::app::HotkeySlot) -> HWND {
    match slot {
        crate::app::HotkeySlot::LastWord => state.hotkeys.last_word,
        crate::app::HotkeySlot::Pause => state.hotkeys.pause,
        crate::app::HotkeySlot::Selection => state.hotkeys.selection,
        crate::app::HotkeySlot::SwitchLayout => state.hotkeys.switch_layout,
    }
}
