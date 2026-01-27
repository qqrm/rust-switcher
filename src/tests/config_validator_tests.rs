use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

use crate::config::{
    HotkeyChord, HotkeySequence, RawConfig,
    constants::{CONVERT_LAST_WORD, CONVERT_SELECTION, PAUSE, SWITCH_LAYOUT},
};

fn chord(mods: u32, mods_vks: u32, vk: u32) -> HotkeyChord {
    HotkeyChord {
        mods,
        mods_vks,
        vk: Some(vk),
    }
}

fn seq1(mods: u32, vk: u32) -> HotkeySequence {
    HotkeySequence {
        first: chord(mods, 0, vk),
        second: None,
        max_gap_ms: 250,
    }
}

fn seq1_gap(mods: u32, vk: u32, max_gap_ms: u32) -> HotkeySequence {
    HotkeySequence {
        first: chord(mods, 0, vk),
        second: None,
        max_gap_ms,
    }
}

fn seq1_modsvks(mods: u32, mods_vks: u32, vk: u32) -> HotkeySequence {
    HotkeySequence {
        first: chord(mods, mods_vks, vk),
        second: None,
        max_gap_ms: 250,
    }
}

fn seq2(mods1: u32, vk1: u32, mods2: u32, vk2: u32, max_gap_ms: u32) -> HotkeySequence {
    HotkeySequence {
        first: chord(mods1, 0, vk1),
        second: Some(chord(mods2, 0, vk2)),
        max_gap_ms,
    }
}

fn mk_cfg(
    last_word: Option<HotkeySequence>,
    pause: Option<HotkeySequence>,
    selection: Option<HotkeySequence>,
    layout: Option<HotkeySequence>,
) -> RawConfig {
    RawConfig {
        hotkey_convert_last_word_sequence: last_word,
        hotkey_pause_sequence: pause,
        hotkey_convert_selection_sequence: selection,
        hotkey_switch_layout_sequence: layout,
        ..Default::default()
    }
}

fn assert_ok(cfg: RawConfig) {
    let res = cfg.validate_hotkey_sequences();
    assert!(res.is_ok(), "expected Ok(()), got Err: {res:?}");
}

fn assert_err(cfg: RawConfig) -> String {
    match cfg.validate_hotkey_sequences() {
        Ok(()) => panic!("expected Err, got Ok(())"),
        Err(e) => e,
    }
}

fn assert_has_common_error_shape(err: &str) {
    assert!(
        err.starts_with("Duplicate hotkey sequences found:\n\n"),
        "bad header: {err}"
    );
    assert!(
        err.contains("\nEach action must have a unique hotkey sequence."),
        "missing footer: {err}"
    );
    assert!(err.contains("• '"), "missing bullet formatting: {err}");
}

#[test]
fn no_sequences_ok() {
    assert_ok(mk_cfg(None, None, None, None));
}

#[test]
fn only_one_sequence_ok() {
    assert_ok(mk_cfg(
        Some(seq1(MOD_CONTROL.0, u32::from(b'A'))),
        None,
        None,
        None,
    ));
}

#[test]
fn no_duplicates_ok() {
    assert_ok(mk_cfg(
        Some(seq1(MOD_CONTROL.0, u32::from(b'A'))),
        Some(seq1(MOD_ALT.0, u32::from(b'B'))),
        Some(seq1(MOD_SHIFT.0, u32::from(b'C'))),
        Some(seq1(MOD_WIN.0, u32::from(b'D'))),
    ));
}

#[test]
fn allowed_duplicate_last_word_and_selection_ok() {
    let same = seq1(MOD_CONTROL.0, u32::from(b'X'));
    assert_ok(mk_cfg(
        Some(same),
        Some(seq1(MOD_ALT.0, u32::from(b'B'))),
        Some(seq1(MOD_CONTROL.0, u32::from(b'X'))),
        Some(seq1(MOD_SHIFT.0, u32::from(b'C'))),
    ));
}

#[test]
fn duplicate_pause_and_layout_err() {
    let dup = seq1(MOD_ALT.0, u32::from(b'B'));

    let err = assert_err(mk_cfg(
        Some(seq1(MOD_CONTROL.0, u32::from(b'A'))),
        Some(dup),
        Some(seq1(MOD_SHIFT.0, u32::from(b'C'))),
        Some(seq1(MOD_ALT.0, u32::from(b'B'))),
    ));

    assert_has_common_error_shape(&err);
    assert!(err.contains(PAUSE), "{err}");
    assert!(err.contains(SWITCH_LAYOUT), "{err}");
    assert!(
        err.contains(&format!("• '{PAUSE}' and '{SWITCH_LAYOUT}'\n")),
        "{err}"
    );
}

#[test]
fn duplicate_last_word_and_pause_err() {
    let dup = seq1(MOD_CONTROL.0, u32::from(b'A'));

    let err = assert_err(mk_cfg(
        Some(dup),
        Some(seq1(MOD_CONTROL.0, u32::from(b'A'))),
        Some(seq1(MOD_SHIFT.0, u32::from(b'C'))),
        Some(seq1(MOD_ALT.0, u32::from(b'B'))),
    ));

    assert_has_common_error_shape(&err);
    assert!(err.contains(CONVERT_LAST_WORD), "{err}");
    assert!(err.contains(PAUSE), "{err}");
    assert!(
        err.contains(&format!("• '{CONVERT_LAST_WORD}' and '{PAUSE}'\n")),
        "{err}"
    );
}

#[test]
fn duplicate_selection_and_pause_err() {
    let dup = seq1(MOD_CONTROL.0, u32::from(b'A'));

    let err = assert_err(mk_cfg(
        Some(seq1(MOD_SHIFT.0, u32::from(b'C'))),
        Some(dup),
        Some(seq1(MOD_CONTROL.0, u32::from(b'A'))),
        Some(seq1(MOD_ALT.0, u32::from(b'B'))),
    ));

    assert_has_common_error_shape(&err);
    assert!(err.contains(CONVERT_SELECTION), "{err}");
    assert!(err.contains(PAUSE), "{err}");
    assert!(
        err.contains(&format!("• '{PAUSE}' and '{CONVERT_SELECTION}'\n")),
        "{err}"
    );
}

#[test]
fn allowed_duplicate_pair_but_third_action_same_still_err_lists_two_pairs() {
    let same = seq1(MOD_CONTROL.0, u32::from(b'X'));

    let err = assert_err(mk_cfg(
        Some(same),
        Some(seq1(MOD_CONTROL.0, u32::from(b'X'))),
        Some(seq1(MOD_CONTROL.0, u32::from(b'X'))),
        None,
    ));

    assert_has_common_error_shape(&err);

    let expected_1 = format!("• '{CONVERT_LAST_WORD}' and '{PAUSE}'\n");
    let expected_2 = format!("• '{PAUSE}' and '{CONVERT_SELECTION}'\n");

    assert!(err.contains(&expected_1), "{err}");
    assert!(err.contains(&expected_2), "{err}");

    let forbidden = format!("• '{CONVERT_LAST_WORD}' and '{CONVERT_SELECTION}'\n");
    assert!(!err.contains(&forbidden), "{err}");
}

#[test]
fn two_independent_duplicate_pairs_err_lists_both_in_stable_order() {
    let a = seq1(MOD_CONTROL.0, u32::from(b'A'));
    let b = seq1(MOD_ALT.0, u32::from(b'B'));

    let err = assert_err(mk_cfg(
        Some(a),
        Some(seq1(MOD_CONTROL.0, u32::from(b'A'))),
        Some(b),
        Some(seq1(MOD_ALT.0, u32::from(b'B'))),
    ));

    assert_has_common_error_shape(&err);

    let expected = format!(
        "Duplicate hotkey sequences found:\n\n• '{CONVERT_LAST_WORD}' and '{PAUSE}'\n• '{CONVERT_SELECTION}' and '{SWITCH_LAYOUT}'\n\nEach action must have a unique hotkey sequence."
    );

    assert_eq!(err, expected);
}

#[test]
fn duplicates_across_non_adjacent_actions_err() {
    let dup = seq1(MOD_SHIFT.0, u32::from(b'Z'));

    let err = assert_err(mk_cfg(
        Some(dup),
        None,
        Some(seq1(MOD_CONTROL.0, u32::from(b'A'))),
        Some(seq1(MOD_SHIFT.0, u32::from(b'Z'))),
    ));

    assert_has_common_error_shape(&err);
    assert!(
        err.contains(&format!("• '{CONVERT_LAST_WORD}' and '{SWITCH_LAYOUT}'\n")),
        "{err}"
    );
}

#[test]
fn different_max_gap_is_not_duplicate_current_behavior() {
    let s1 = seq1_gap(MOD_CONTROL.0, u32::from(b'K'), 200);
    let s2 = seq1_gap(MOD_CONTROL.0, u32::from(b'K'), 400);

    assert_ok(mk_cfg(Some(s1), Some(s2), None, None));
}

#[test]
fn different_mods_vks_is_not_duplicate_current_behavior() {
    let s1 = seq1_modsvks(MOD_CONTROL.0, 0, u32::from(b'K'));
    let s2 = seq1_modsvks(MOD_CONTROL.0, 1, u32::from(b'K'));

    assert_ok(mk_cfg(Some(s1), Some(s2), None, None));
}

#[test]
fn different_second_chord_is_not_duplicate_current_behavior() {
    let s1 = seq2(
        MOD_CONTROL.0,
        u32::from(b'A'),
        MOD_SHIFT.0,
        u32::from(b'B'),
        250,
    );
    let s2 = seq2(
        MOD_CONTROL.0,
        u32::from(b'A'),
        MOD_SHIFT.0,
        u32::from(b'C'),
        250,
    );

    assert_ok(mk_cfg(Some(s1), Some(s2), None, None));
}

#[test]
fn same_two_chord_sequence_is_duplicate_err() {
    let s = seq2(
        MOD_CONTROL.0,
        u32::from(b'A'),
        MOD_SHIFT.0,
        u32::from(b'B'),
        250,
    );

    let err = assert_err(mk_cfg(
        Some(s),
        Some(seq2(
            MOD_CONTROL.0,
            u32::from(b'A'),
            MOD_SHIFT.0,
            u32::from(b'B'),
            250,
        )),
        None,
        None,
    ));

    assert_has_common_error_shape(&err);
    assert!(
        err.contains(&format!("• '{CONVERT_LAST_WORD}' and '{PAUSE}'\n")),
        "{err}"
    );
}

#[test]
fn none_values_are_ignored_when_searching_duplicates() {
    let dup = seq1(MOD_ALT.0, u32::from(b'Q'));

    let err = assert_err(mk_cfg(
        None,
        Some(dup),
        None,
        Some(seq1(MOD_ALT.0, u32::from(b'Q'))),
    ));

    assert_has_common_error_shape(&err);
    assert!(err.contains(PAUSE), "{err}");
    assert!(err.contains(SWITCH_LAYOUT), "{err}");
}

#[test]
fn error_message_includes_only_unique_pairs_once() {
    let s = seq1(MOD_CONTROL.0, u32::from(b'X'));

    let err = assert_err(mk_cfg(
        Some(s),
        Some(seq1(MOD_CONTROL.0, u32::from(b'X'))),
        Some(seq1(MOD_CONTROL.0, u32::from(b'X'))),
        None,
    ));

    let bullets: Vec<&str> = err.lines().filter(|l| l.starts_with("• '")).collect();

    assert_eq!(
        bullets.len(),
        2,
        "expected exactly 2 bullet lines, got {bullets:?}\n{err}"
    );
}
