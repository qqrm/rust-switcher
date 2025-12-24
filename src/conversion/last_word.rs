use std::{thread, time::Duration};

use windows::Win32::UI::{
    Input::KeyboardAndMouse::VIRTUAL_KEY, WindowsAndMessaging::GetForegroundWindow,
};

use super::{mapping::convert_ru_en_bidirectional, switch_keyboard_layout, wait_shift_released};
use crate::{
    app::AppState,
    conversion::input::{KeySequence, send_text_unicode},
};

const VK_BACKSPACE_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x08);
const VK_LEFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x25);
const VK_RIGHT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x27);

/// Converts the last typed word using the input journal and replaces it in the active editor.
///
/// Suffix policy:
/// - if `suffix` contains `\n` or `\r`, conversion is skipped (multi line context)
/// - if `suffix` consists only of spaces and tabs, whitespace is preserved in place:
///   the caret is moved left over whitespace, the word is replaced, then the caret is moved back
/// - otherwise, `word + suffix` is deleted and reinserted as `converted + suffix`
///
/// All UI notifications and layout switching are best effort. Failures are logged.
#[tracing::instrument(level = "trace", skip(state))]
pub fn convert_last_word(state: &mut AppState) {
    if !foreground_window_alive() {
        tracing::warn!("foreground window is null");
        return;
    }

    if !wait_shift_released(150) {
        tracing::info!("wait_shift_released returned false");
        return;
    }

    sleep_before_convert(state);

    let Some(payload) = take_last_word_payload() else {
        tracing::info!("journal: no last word");
        return;
    };

    if payload.suffix_has_newline {
        tracing::trace!("suffix contains newline, skipping convert_last_word");
        return;
    }

    let converted = convert_ru_en_bidirectional(&payload.word);
    tracing::trace!(%converted, "converted");

    let mut seq = KeySequence::new();
    if !apply_last_word_conversion(&mut seq, &payload, &converted) {
        tracing::warn!("apply_last_word_conversion failed");
        return;
    }

    update_journal(&payload, &converted);

    match switch_keyboard_layout() {
        Ok(()) => tracing::trace!("layout switched"),
        Err(e) => tracing::warn!(error = ?e, "layout switch failed"),
    }
}

fn foreground_window_alive() -> bool {
    let fg = unsafe { GetForegroundWindow() };
    !fg.0.is_null()
}

/// Sleeps for the configured `delay_ms` before injecting synthetic input.
///
/// This reduces races with the target application after the hotkey trigger.
fn sleep_before_convert(state: &AppState) {
    let delay_ms = crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100);
    tracing::trace!(delay_ms, "sleep before convert");
    thread::sleep(Duration::from_millis(delay_ms as u64));
}

/// Normalized data needed for last word conversion.
struct LastWordPayload {
    word: String,
    suffix: String,
    word_len: usize,
    suffix_len: usize,
    suffix_spaces_only: bool,
    suffix_has_newline: bool,
}

/// Extracts the last "word" and its trailing suffix from the input journal, with a small fixup:
/// if the suffix begins with a convertible punctuation character (`?`, `/`, `,`, `.`) and the rest
/// of the suffix contains only spaces or tabs, that punctuation is moved into the word so it gets
/// converted together with the word during "Convert Last Word".
///
/// This prevents cases like `ghbdtn?` from leaving `?` unconverted when converting only the last word.
/// Newlines in the suffix are not merged into the word.
/// Reads `(word, suffix)` from the input journal and derives a normalized payload.
/// Returns `None` if the journal is empty or the extracted `word` is empty.
fn take_last_word_payload() -> Option<LastWordPayload> {
    fn is_convertible_trailing_punct(ch: char) -> bool {
        matches!(ch, '?' | '/' | ',' | '.')
    }

    crate::input_journal::take_last_word_with_suffix().and_then(|(mut word, mut suffix)| {
        if word.is_empty() {
            return None;
        }

        // If suffix starts with convertible punctuation and the rest is only spaces or tabs (or empty),
        // move that punctuation into the word so it gets converted together with the word.
        let (first, rest) = match suffix.chars().next() {
            Some(ch) if is_convertible_trailing_punct(ch) => {
                let ch_len = ch.len_utf8();
                (Some(ch), &suffix[ch_len..])
            }
            _ => (None, ""),
        };

        if let Some(ch) = first
            && rest.chars().all(|c| c == ' ' || c == '\t')
        {
            word.push(ch);
            suffix = rest.to_string();
        }

        let word_len = word.chars().count();
        let suffix_len = suffix.chars().count();
        let suffix_has_newline = suffix.contains('\n') || suffix.contains('\r');
        let suffix_spaces_only =
            !suffix.is_empty() && suffix.chars().all(|c| c == ' ' || c == '\t');

        tracing::trace!(
            %word,
            %suffix,
            word_len,
            suffix_len,
            suffix_spaces_only,
            suffix_has_newline,
            "journal extracted"
        );

        Some(LastWordPayload {
            word,
            suffix,
            word_len,
            suffix_len,
            suffix_spaces_only,
            suffix_has_newline,
        })
    })
}

/// Applies conversion according to the suffix policy.
///
/// Returns `true` if all required synthetic input events were sent successfully.
fn apply_last_word_conversion(seq: &mut KeySequence, p: &LastWordPayload, converted: &str) -> bool {
    const MAX_TAPS: usize = 4096;

    let word_len = p.word_len.min(MAX_TAPS);
    let suffix_len = p.suffix_len.min(MAX_TAPS);

    if p.suffix_spaces_only {
        move_caret_left(seq, suffix_len)
            && delete_with_backspace(seq, word_len)
            && send_text_unicode(converted)
            && move_caret_right(seq, suffix_len)
    } else {
        let delete_count = p.word_len.saturating_add(p.suffix_len).min(MAX_TAPS);

        delete_with_backspace(seq, delete_count)
            && send_text_unicode(converted)
            && (p.suffix.is_empty() || send_text_unicode(&p.suffix))
    }
}

/// Updates the input journal to match what was inserted.
fn update_journal(p: &LastWordPayload, converted: &str) {
    crate::input_journal::push_text(converted);
    if !p.suffix.is_empty() {
        crate::input_journal::push_text(&p.suffix);
    }
    tracing::trace!("journal updated");
}

/// Sends Backspace taps `count` times.
fn delete_with_backspace(seq: &mut KeySequence, count: usize) -> bool {
    repeat_tap(seq, VK_BACKSPACE_KEY, count, "backspace tap failed")
}

/// Moves caret left by sending Left Arrow taps `count` times.
fn move_caret_left(seq: &mut KeySequence, count: usize) -> bool {
    repeat_tap(seq, VK_LEFT_KEY, count, "left arrow tap failed")
}

/// Moves caret right by sending Right Arrow taps `count` times.
fn move_caret_right(seq: &mut KeySequence, count: usize) -> bool {
    repeat_tap(seq, VK_RIGHT_KEY, count, "right arrow tap failed")
}

/// Repeats `seq.tap(vk)` `count` times, logging the first failing iteration.
fn repeat_tap(seq: &mut KeySequence, vk: VIRTUAL_KEY, count: usize, err_msg: &'static str) -> bool {
    (0..count)
        .try_for_each(|i| seq.tap(vk).then_some(()).ok_or(i))
        .map(|_| true)
        .unwrap_or_else(|i| {
            tracing::error!(i, count, %err_msg);
            false
        })
}
