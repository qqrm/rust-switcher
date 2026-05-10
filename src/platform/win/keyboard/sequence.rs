use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_HOTKEY},
};

use crate::{
    app::HotkeySlot,
    config,
    input::hotkeys::{
        HK_CONVERT_LAST_SEQUENCE_ID, HK_CONVERT_LAST_WORD_ID, HK_CONVERT_SELECTION_ID,
        HK_PAUSE_TOGGLE_ID, HK_SWITCH_LAYOUT_ID,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceMatch {
    NoMatch,
    Consumed,
    Triggered(HotkeySlot),
}

pub(crate) fn chord_matches(template: config::HotkeyChord, input: config::HotkeyChord) -> bool {
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

pub(crate) fn progress_for_slot_mut(
    state: &mut crate::app::AppState,
    slot: crate::app::HotkeySlot,
) -> &mut crate::app::SequenceProgress {
    match slot {
        HotkeySlot::LastWord => &mut state.hotkey_sequence_progress.last_word,
        HotkeySlot::LastSequence => &mut state.hotkey_sequence_progress.last_sequence,
        HotkeySlot::Pause => &mut state.hotkey_sequence_progress.pause,
        HotkeySlot::Selection => &mut state.hotkey_sequence_progress.selection,
        HotkeySlot::SwitchLayout => &mut state.hotkey_sequence_progress.switch_layout,
    }
}

pub(crate) fn hotkey_id_for_slot(slot: crate::app::HotkeySlot) -> i32 {
    match slot {
        HotkeySlot::LastWord => HK_CONVERT_LAST_WORD_ID,
        HotkeySlot::LastSequence => HK_CONVERT_LAST_SEQUENCE_ID,
        HotkeySlot::Pause => HK_PAUSE_TOGGLE_ID,
        HotkeySlot::Selection => HK_CONVERT_SELECTION_ID,
        HotkeySlot::SwitchLayout => HK_SWITCH_LAYOUT_ID,
    }
}

pub(crate) fn effective_gap_ms(_slot: crate::app::HotkeySlot, seq: config::HotkeySequence) -> u64 {
    u64::from(seq.max_gap_ms)
}

pub(crate) fn post_hotkey(hwnd: HWND, id: i32) -> windows::core::Result<()> {
    let id_usize = usize::try_from(id).map_err(|_| {
        windows::core::Error::new(
            windows::core::HRESULT(0x8007_0057_u32.cast_signed()),
            "hotkey id must be non-negative",
        )
    })?;

    unsafe { PostMessageW(Some(hwnd), WM_HOTKEY, WPARAM(id_usize), LPARAM(0)) }
}

pub(crate) fn try_match_sequence(
    state: &mut crate::app::AppState,
    slot: crate::app::HotkeySlot,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> SequenceMatch {
    let Some(seq) = state.active_hotkey_sequences.get(slot) else {
        return SequenceMatch::NoMatch;
    };

    let first = seq.first;

    // Single chord
    let Some(second) = seq.second else {
        if chord_matches(first, chord) {
            return SequenceMatch::Triggered(slot);
        }
        return SequenceMatch::NoMatch;
    };

    let gap_ms = effective_gap_ms(slot, seq);
    let prog = progress_for_slot_mut(state, slot);

    if prog.waiting_second {
        let elapsed_ms = now_ms.saturating_sub(prog.first_tick_ms);
        if elapsed_ms > gap_ms {
            prog.waiting_second = false;
            prog.first_tick_ms = 0;
        }
    }

    if prog.waiting_second {
        if chord_matches(second, chord) {
            prog.waiting_second = false;
            prog.first_tick_ms = 0;

            return SequenceMatch::Triggered(slot);
        }

        if chord_matches(first, chord) {
            prog.first_tick_ms = now_ms;
            return SequenceMatch::Consumed;
        }

        prog.waiting_second = false;
        prog.first_tick_ms = 0;
        return SequenceMatch::NoMatch;
    }

    if chord_matches(first, chord) {
        prog.waiting_second = true;
        prog.first_tick_ms = now_ms;
        return SequenceMatch::Consumed;
    }

    SequenceMatch::NoMatch
}

pub(crate) fn try_match_any_sequence_slot(
    state: &mut crate::app::AppState,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> SequenceMatch {
    for slot in [
        crate::app::HotkeySlot::SwitchLayout,
        crate::app::HotkeySlot::LastWord,
        crate::app::HotkeySlot::LastSequence,
        crate::app::HotkeySlot::Selection,
        crate::app::HotkeySlot::Pause,
    ] {
        let result = try_match_sequence(state, slot, chord, now_ms);
        if result != SequenceMatch::NoMatch {
            return result;
        }
    }

    SequenceMatch::NoMatch
}

pub(crate) fn try_match_any_sequence(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> windows::core::Result<bool> {
    match try_match_any_sequence_slot(state, chord, now_ms) {
        SequenceMatch::NoMatch => Ok(false),
        SequenceMatch::Consumed => Ok(true),
        SequenceMatch::Triggered(slot) => {
            post_hotkey(hwnd, hotkey_id_for_slot(slot))?;
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_SHIFT};

    use super::*;
    use crate::{
        app::{AppState, HotkeySequenceValues},
        config::{HotkeyChord, HotkeySequence, MODVK_LALT, MODVK_LSHIFT},
    };

    fn modifier_only_chord(mods: u32, mods_vks: u32) -> HotkeyChord {
        HotkeyChord {
            mods,
            mods_vks,
            vk: None,
        }
    }

    fn double_modifier_sequence(mods: u32, mods_vks: u32) -> HotkeySequence {
        HotkeySequence {
            first: modifier_only_chord(mods, mods_vks),
            second: Some(modifier_only_chord(mods, mods_vks)),
            max_gap_ms: 1000,
        }
    }

    fn state_with_sequences(sequences: HotkeySequenceValues) -> AppState {
        AppState {
            active_hotkey_sequences: sequences,
            ..Default::default()
        }
    }

    #[test]
    fn double_left_alt_triggers_last_sequence() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_sequence: Some(double_modifier_sequence(MOD_ALT.0, MODVK_LALT)),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_ALT.0, MODVK_LALT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 250),
            SequenceMatch::Triggered(HotkeySlot::LastSequence)
        );
    }

    #[test]
    fn double_left_alt_respects_gap_timeout() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_sequence: Some(double_modifier_sequence(MOD_ALT.0, MODVK_LALT)),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_ALT.0, MODVK_LALT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 1201),
            SequenceMatch::Consumed
        );
    }

    #[test]
    fn shared_double_left_shift_prioritizes_last_word_before_selection() {
        let shared = double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT);
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(shared),
            selection: Some(shared),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 250),
            SequenceMatch::Triggered(HotkeySlot::LastWord)
        );
    }

    #[test]
    fn single_chord_switch_layout_triggers_immediately() {
        let chord = HotkeyChord {
            mods: 0,
            mods_vks: 0,
            vk: Some(20),
        };
        let mut state = state_with_sequences(HotkeySequenceValues {
            switch_layout: Some(HotkeySequence {
                first: chord,
                second: None,
                max_gap_ms: 1000,
            }),
            ..Default::default()
        });

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Triggered(HotkeySlot::SwitchLayout)
        );
    }

    #[test]
    fn mismatched_second_chord_resets_sequence_and_allows_retry() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_sequence: Some(double_modifier_sequence(MOD_ALT.0, MODVK_LALT)),
            ..Default::default()
        });
        let left_alt = modifier_only_chord(MOD_ALT.0, MODVK_LALT);
        let wrong = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_alt, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, wrong, 200),
            SequenceMatch::NoMatch
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_alt, 300),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_alt, 400),
            SequenceMatch::Triggered(HotkeySlot::LastSequence)
        );
    }
}
