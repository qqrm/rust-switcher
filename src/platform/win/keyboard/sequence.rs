use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{KillTimer, PostMessageW, SetTimer, WM_HOTKEY},
};

use crate::{
    app::{DeferredSequenceHotkey, HotkeySlot},
    config,
    input::hotkeys::{
        HK_CONVERT_LAST_SEQUENCE_ID, HK_CONVERT_LAST_WORD_ID, HK_CONVERT_SELECTION_ID,
        HK_PAUSE_TOGGLE_ID, HK_SMART_CONVERT_LAST_SEQUENCE_ID, HK_SMART_CONVERT_LAST_WORD_ID,
        HK_SMART_CONVERT_SELECTION_ID, HK_SWITCH_LAYOUT_ID,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceMatch {
    NoMatch,
    Consumed,
    Pending(HotkeySlot, u32),
    Triggered(HotkeySlot),
}

pub(crate) const DEFERRED_SEQUENCE_TIMER_ID: usize = 0x5301;

const SLOT_ORDER: [HotkeySlot; 8] = [
    HotkeySlot::SwitchLayout,
    HotkeySlot::SmartLastWord,
    HotkeySlot::SmartLastSequence,
    HotkeySlot::SmartSelection,
    HotkeySlot::LastWord,
    HotkeySlot::LastSequence,
    HotkeySlot::Selection,
    HotkeySlot::Pause,
];

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
        HotkeySlot::SmartLastWord => &mut state.hotkey_sequence_progress.smart_last_word,
        HotkeySlot::SmartLastSequence => &mut state.hotkey_sequence_progress.smart_last_sequence,
        HotkeySlot::SmartSelection => &mut state.hotkey_sequence_progress.smart_selection,
    }
}

fn progress_for_slot(
    state: &crate::app::AppState,
    slot: crate::app::HotkeySlot,
) -> &crate::app::SequenceProgress {
    match slot {
        HotkeySlot::LastWord => &state.hotkey_sequence_progress.last_word,
        HotkeySlot::LastSequence => &state.hotkey_sequence_progress.last_sequence,
        HotkeySlot::Pause => &state.hotkey_sequence_progress.pause,
        HotkeySlot::Selection => &state.hotkey_sequence_progress.selection,
        HotkeySlot::SwitchLayout => &state.hotkey_sequence_progress.switch_layout,
        HotkeySlot::SmartLastWord => &state.hotkey_sequence_progress.smart_last_word,
        HotkeySlot::SmartLastSequence => &state.hotkey_sequence_progress.smart_last_sequence,
        HotkeySlot::SmartSelection => &state.hotkey_sequence_progress.smart_selection,
    }
}

pub(crate) fn hotkey_id_for_slot(slot: crate::app::HotkeySlot) -> i32 {
    match slot {
        HotkeySlot::LastWord => HK_CONVERT_LAST_WORD_ID,
        HotkeySlot::LastSequence => HK_CONVERT_LAST_SEQUENCE_ID,
        HotkeySlot::Pause => HK_PAUSE_TOGGLE_ID,
        HotkeySlot::Selection => HK_CONVERT_SELECTION_ID,
        HotkeySlot::SwitchLayout => HK_SWITCH_LAYOUT_ID,
        HotkeySlot::SmartLastWord => HK_SMART_CONVERT_LAST_WORD_ID,
        HotkeySlot::SmartLastSequence => HK_SMART_CONVERT_LAST_SEQUENCE_ID,
        HotkeySlot::SmartSelection => HK_SMART_CONVERT_SELECTION_ID,
    }
}

pub(crate) fn effective_gap_ms(_slot: crate::app::HotkeySlot, seq: config::HotkeySequence) -> u64 {
    u64::from(seq.max_gap_ms)
}

fn sequence_chord_count(seq: config::HotkeySequence) -> usize {
    [Some(seq.first), seq.second, seq.third]
        .iter()
        .flatten()
        .count()
}

fn sequence_chords(seq: config::HotkeySequence) -> [Option<config::HotkeyChord>; 3] {
    [Some(seq.first), seq.second, seq.third]
}

fn sequence_has_prefix(
    longer: config::HotkeySequence,
    prefix: config::HotkeySequence,
) -> bool {
    let longer_chords = sequence_chords(longer);
    let prefix_chords = sequence_chords(prefix);
    let prefix_len = sequence_chord_count(prefix);

    if sequence_chord_count(longer) <= prefix_len {
        return false;
    }

    longer_chords
        .iter()
        .zip(prefix_chords.iter())
        .take(prefix_len)
        .all(|(longer, prefix)| match (longer, prefix) {
            (Some(longer), Some(prefix)) => chord_matches(*longer, *prefix),
            _ => false,
        })
}

fn priority_index(slot: HotkeySlot) -> usize {
    SLOT_ORDER
        .iter()
        .position(|candidate| *candidate == slot)
        .unwrap_or(SLOT_ORDER.len())
}

fn clear_all_sequence_progress(state: &mut crate::app::AppState) {
    state.hotkey_sequence_progress = crate::app::HotkeySequenceProgress::default();
}

fn clear_deferred_sequence(hwnd: HWND, state: &mut crate::app::AppState) {
    state.deferred_sequence_hotkey = None;
    clear_all_sequence_progress(state);
    unsafe {
        let _ = KillTimer(Some(hwnd), DEFERRED_SEQUENCE_TIMER_ID);
    }
}

fn chord_can_continue_deferred_sequence(
    state: &crate::app::AppState,
    deferred_slot: HotkeySlot,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> bool {
    let Some(deferred_seq) = state.active_hotkey_sequences.get(deferred_slot) else {
        return false;
    };
    let deferred_len = sequence_chord_count(deferred_seq);

    SLOT_ORDER.iter().copied().any(|slot| {
        let Some(seq) = state.active_hotkey_sequences.get(slot) else {
            return false;
        };
        if !sequence_has_prefix(seq, deferred_seq) {
            return false;
        }

        let progress = progress_for_slot(state, slot);
        if usize::from(progress.matched_chords) < deferred_len {
            return false;
        }
        if now_ms.saturating_sub(progress.first_tick_ms) >= effective_gap_ms(slot, seq) {
            return false;
        }

        sequence_chords(seq)
            .get(usize::from(progress.matched_chords))
            .copied()
            .flatten()
            .is_some_and(|next| chord_matches(next, chord))
    })
}

fn chord_can_match_any_sequence_now(
    state: &crate::app::AppState,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> bool {
    SLOT_ORDER.iter().copied().any(|slot| {
        let Some(seq) = state.active_hotkey_sequences.get(slot) else {
            return false;
        };

        let progress = progress_for_slot(state, slot);
        if progress.matched_chords > 0
            && now_ms.saturating_sub(progress.first_tick_ms) < effective_gap_ms(slot, seq)
            && sequence_chords(seq)
                .get(usize::from(progress.matched_chords))
                .copied()
                .flatten()
                .is_some_and(|next| chord_matches(next, chord))
        {
            return true;
        }

        chord_matches(seq.first, chord)
    })
}

pub(crate) fn take_deferred_sequence_disambiguated_by_chord(
    state: &mut crate::app::AppState,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> Option<HotkeySlot> {
    let deferred = state.deferred_sequence_hotkey?;
    if !chord_can_match_any_sequence_now(state, chord, now_ms) {
        return None;
    }
    if chord_can_continue_deferred_sequence(state, deferred.slot, chord, now_ms) {
        return None;
    }

    state.deferred_sequence_hotkey = None;
    Some(deferred.slot)
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

    let chords = [Some(seq.first), seq.second, seq.third];
    let chord_count = chords.iter().flatten().count();

    if chord_count == 1 {
        if chord_matches(seq.first, chord) {
            return SequenceMatch::Triggered(slot);
        }
        return SequenceMatch::NoMatch;
    }

    let gap_ms = effective_gap_ms(slot, seq);
    let prog = progress_for_slot_mut(state, slot);

    if prog.matched_chords > 0 {
        let elapsed_ms = now_ms.saturating_sub(prog.first_tick_ms);
        if elapsed_ms >= gap_ms {
            *prog = crate::app::SequenceProgress::default();
        }
    }

    if prog.matched_chords > 0 {
        let next_idx = usize::from(prog.matched_chords);
        let Some(next) = chords.get(next_idx).copied().flatten() else {
            *prog = crate::app::SequenceProgress::default();
            return SequenceMatch::NoMatch;
        };

        if chord_matches(next, chord) {
            prog.matched_chords += 1;
            prog.waiting_second = prog.matched_chords < chord_count as u8;

            if usize::from(prog.matched_chords) == chord_count {
                *prog = crate::app::SequenceProgress::default();
                return SequenceMatch::Triggered(slot);
            }

            prog.first_tick_ms = now_ms;
            return SequenceMatch::Consumed;
        }

        if chord_matches(seq.first, chord) {
            prog.matched_chords = 1;
            prog.waiting_second = true;
            prog.first_tick_ms = now_ms;
            return SequenceMatch::Consumed;
        }

        *prog = crate::app::SequenceProgress::default();
        return SequenceMatch::NoMatch;
    }

    if chord_matches(seq.first, chord) {
        prog.matched_chords = 1;
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
    let mut consumed_slots = Vec::new();
    let mut triggered_slots = Vec::new();

    for slot in SLOT_ORDER {
        let result = try_match_sequence(state, slot, chord, now_ms);
        match result {
            SequenceMatch::NoMatch => {}
            SequenceMatch::Consumed => consumed_slots.push(slot),
            SequenceMatch::Triggered(slot) => {
                triggered_slots.push(slot);
                break;
            }
            SequenceMatch::Pending(_, _) => unreachable!("per-slot matching cannot defer"),
        }
    }

    if let Some(triggered) = triggered_slots
        .iter()
        .copied()
        .min_by_key(|slot| priority_index(*slot))
    {
        let Some(triggered_seq) = state.active_hotkey_sequences.get(triggered) else {
            return SequenceMatch::Triggered(triggered);
        };
        let triggered_len = sequence_chord_count(triggered_seq);
        let has_longer_waiting_prefix = consumed_slots.iter().copied().any(|slot| {
            state
                .active_hotkey_sequences
                .get(slot)
                .is_some_and(|seq| sequence_chord_count(seq) > triggered_len)
        });

        if has_longer_waiting_prefix {
            let delay_ms = triggered_seq.max_gap_ms;
            state.deferred_sequence_hotkey = Some(DeferredSequenceHotkey { slot: triggered });
            return SequenceMatch::Pending(triggered, delay_ms);
        }

        state.deferred_sequence_hotkey = None;
        clear_all_sequence_progress(state);
        return SequenceMatch::Triggered(triggered);
    }

    if consumed_slots.is_empty() {
        SequenceMatch::NoMatch
    } else {
        SequenceMatch::Consumed
    }
}

pub(crate) fn try_match_any_sequence(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    chord: config::HotkeyChord,
    now_ms: u64,
) -> windows::core::Result<bool> {
    if let Some(slot) = take_deferred_sequence_disambiguated_by_chord(state, chord, now_ms) {
        unsafe {
            let _ = KillTimer(Some(hwnd), DEFERRED_SEQUENCE_TIMER_ID);
        }
        post_hotkey(hwnd, hotkey_id_for_slot(slot))?;
    }

    match try_match_any_sequence_slot(state, chord, now_ms) {
        SequenceMatch::NoMatch => Ok(false),
        SequenceMatch::Consumed => Ok(true),
        SequenceMatch::Pending(_, delay_ms) => {
            unsafe {
                let _ = KillTimer(Some(hwnd), DEFERRED_SEQUENCE_TIMER_ID);
                let _ = SetTimer(Some(hwnd), DEFERRED_SEQUENCE_TIMER_ID, delay_ms, None);
            }
            Ok(true)
        }
        SequenceMatch::Triggered(slot) => {
            clear_deferred_sequence(hwnd, state);
            post_hotkey(hwnd, hotkey_id_for_slot(slot))?;
            Ok(true)
        }
    }
}

pub(crate) fn handle_deferred_sequence_timer(
    hwnd: HWND,
    state: &mut crate::app::AppState,
) -> windows::core::Result<()> {
    unsafe {
        let _ = KillTimer(Some(hwnd), DEFERRED_SEQUENCE_TIMER_ID);
    }
    let Some(deferred) = state.deferred_sequence_hotkey.take() else {
        return Ok(());
    };
    clear_all_sequence_progress(state);
    post_hotkey(hwnd, hotkey_id_for_slot(deferred.slot))
}

#[cfg(test)]
mod tests {
    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT};

    use super::*;
    use crate::{
        app::{AppState, HotkeySequenceValues},
        config::{HotkeyChord, HotkeySequence, MODVK_LALT, MODVK_LSHIFT, MODVK_RCTRL},
    };

    fn modifier_only_chord(mods: u32, mods_vks: u32) -> HotkeyChord {
        HotkeyChord {
            mods,
            mods_vks,
            vk: None,
        }
    }

    fn letter_chord(ch: u8) -> HotkeyChord {
        HotkeyChord {
            mods: 0,
            mods_vks: 0,
            vk: Some(u32::from(ch)),
        }
    }

    fn double_modifier_sequence(mods: u32, mods_vks: u32) -> HotkeySequence {
        HotkeySequence {
            first: modifier_only_chord(mods, mods_vks),
            second: Some(modifier_only_chord(mods, mods_vks)),
            third: None,
            max_gap_ms: 1000,
        }
    }

    fn triple_modifier_sequence(mods: u32, mods_vks: u32) -> HotkeySequence {
        HotkeySequence {
            first: modifier_only_chord(mods, mods_vks),
            second: Some(modifier_only_chord(mods, mods_vks)),
            third: Some(modifier_only_chord(mods, mods_vks)),
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
    fn double_left_alt_resets_at_one_second_gap() {
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
            try_match_any_sequence_slot(&mut state, chord, 1100),
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
    fn shared_double_left_shift_clears_lower_priority_selection_progress_after_trigger() {
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
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 400),
            SequenceMatch::Consumed
        );
    }

    #[test]
    fn shift_shift_then_alt_alt_triggers_word_then_sequence() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            last_sequence: Some(double_modifier_sequence(MOD_ALT.0, MODVK_LALT)),
            ..Default::default()
        });
        let left_shift = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);
        let left_alt = modifier_only_chord(MOD_ALT.0, MODVK_LALT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_shift, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_shift, 200),
            SequenceMatch::Triggered(HotkeySlot::LastWord)
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_alt, 300),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_alt, 400),
            SequenceMatch::Triggered(HotkeySlot::LastSequence)
        );
        assert!(!state.hotkey_sequence_progress.last_word.waiting_second);
        assert!(!state.hotkey_sequence_progress.last_sequence.waiting_second);
    }

    #[test]
    fn triple_left_shift_triggers_smart_last_word_on_third_tap() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            smart_last_word: Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 400),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 700),
            SequenceMatch::Triggered(HotkeySlot::SmartLastWord)
        );
    }

    #[test]
    fn smart_triple_shift_preempts_shorter_double_shift_prefix() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            smart_last_word: Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 300),
            SequenceMatch::Pending(HotkeySlot::LastWord, 1000)
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 500),
            SequenceMatch::Triggered(HotkeySlot::SmartLastWord)
        );
    }

    #[test]
    fn triple_left_alt_resets_when_gap_exceeds_one_second() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            smart_last_sequence: Some(triple_modifier_sequence(MOD_ALT.0, MODVK_LALT)),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_ALT.0, MODVK_LALT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 400),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 1501),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 1700),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 1900),
            SequenceMatch::Triggered(HotkeySlot::SmartLastSequence)
        );
    }

    #[test]
    fn double_shift_is_deferred_when_triple_shift_can_still_complete() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            smart_last_word: Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 200),
            SequenceMatch::Pending(HotkeySlot::LastWord, 1000)
        );
        assert_eq!(
            state.deferred_sequence_hotkey.map(|deferred| deferred.slot),
            Some(HotkeySlot::LastWord)
        );
    }

    #[test]
    fn third_shift_cancels_deferred_double_shift_and_triggers_smart() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            smart_last_word: Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 200),
            SequenceMatch::Pending(HotkeySlot::LastWord, 1000)
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 300),
            SequenceMatch::Triggered(HotkeySlot::SmartLastWord)
        );
        assert!(state.deferred_sequence_hotkey.is_none());
    }

    #[test]
    fn shared_triple_left_shift_clears_lower_priority_smart_selection_after_trigger() {
        let triple = triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT);
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            selection: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            smart_last_word: Some(triple),
            smart_selection: Some(triple),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 200),
            SequenceMatch::Pending(HotkeySlot::LastWord, 1000)
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 300),
            SequenceMatch::Triggered(HotkeySlot::SmartLastWord)
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 400),
            SequenceMatch::Consumed
        );
    }

    #[test]
    fn deferred_double_shift_flushes_when_left_alt_starts_another_sequence() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            last_sequence: Some(double_modifier_sequence(MOD_ALT.0, MODVK_LALT)),
            smart_last_word: Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            smart_last_sequence: Some(triple_modifier_sequence(MOD_ALT.0, MODVK_LALT)),
            ..Default::default()
        });
        let left_shift = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);
        let left_alt = modifier_only_chord(MOD_ALT.0, MODVK_LALT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_shift, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_shift, 200),
            SequenceMatch::Pending(HotkeySlot::LastWord, 1000)
        );
        assert_eq!(
            take_deferred_sequence_disambiguated_by_chord(&mut state, left_alt, 300),
            Some(HotkeySlot::LastWord)
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_alt, 300),
            SequenceMatch::Consumed
        );
    }

    #[test]
    fn deferred_double_shift_waits_when_third_shift_can_complete_smart_sequence() {
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            smart_last_word: Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            ..Default::default()
        });
        let chord = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);

        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 200),
            SequenceMatch::Pending(HotkeySlot::LastWord, 1000)
        );
        assert_eq!(
            take_deferred_sequence_disambiguated_by_chord(&mut state, chord, 300),
            None
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, chord, 300),
            SequenceMatch::Triggered(HotkeySlot::SmartLastWord)
        );
    }

    #[test]
    fn deferred_double_shift_does_not_flush_for_chord_that_is_only_a_future_step() {
        let ctrl_then_a = HotkeySequence {
            first: modifier_only_chord(MOD_CONTROL.0, MODVK_RCTRL),
            second: Some(letter_chord(b'A')),
            third: None,
            max_gap_ms: 1000,
        };
        let mut state = state_with_sequences(HotkeySequenceValues {
            last_word: Some(double_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            last_sequence: Some(ctrl_then_a),
            smart_last_word: Some(triple_modifier_sequence(MOD_SHIFT.0, MODVK_LSHIFT)),
            ..Default::default()
        });
        let left_shift = modifier_only_chord(MOD_SHIFT.0, MODVK_LSHIFT);
        let letter_a = letter_chord(b'A');

        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_shift, 100),
            SequenceMatch::Consumed
        );
        assert_eq!(
            try_match_any_sequence_slot(&mut state, left_shift, 200),
            SequenceMatch::Pending(HotkeySlot::LastWord, 1000)
        );
        assert_eq!(
            take_deferred_sequence_disambiguated_by_chord(&mut state, letter_a, 300),
            None
        );
        assert_eq!(
            state.deferred_sequence_hotkey.map(|deferred| deferred.slot),
            Some(HotkeySlot::LastWord)
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
                third: None,
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
