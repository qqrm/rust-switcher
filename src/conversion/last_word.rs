use std::{
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

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

const MIN_WORD_LEN: usize = 4;
const MIN_CONVERTED_CONFIDENCE: f64 = 0.70;
const MIN_CONFIDENCE_GAIN: f64 = 0.25;

static AUTOCONVERT_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn convert_last_word(state: &mut AppState) {
    convert_last_word_impl(state, true);
}

pub fn autoconvert_last_word(state: &mut AppState) {
    if !foreground_window_alive() {
        tracing::warn!("foreground window is null");
        return;
    }

    let _guard = match AutoconvertGuard::try_acquire() {
        Ok(g) => g,
        Err(reason) => {
            tracing::trace!(reason = %reason.as_str(), "autoconvert skip: reentry");
            return;
        }
    };

    sleep_before_convert(state);

    let Some(payload) = take_last_word_payload() else {
        tracing::trace!("journal: no last word");
        return;
    };

    let mut restore = JournalRestore::new(&payload);

    let converted = match autoconvert_candidate(&payload) {
        Ok(v) => v,
        Err(reason) => {
            tracing::trace!(reason = %reason.as_str(), "autoconvert skip: candidate");
            return;
        }
    };

    let detector = language_detector();

    if let Err(reason) = should_autoconvert_word(detector, &payload.word, &converted) {
        tracing::trace!(reason = %reason.as_str(), "autoconvert skip: decision");
        return;
    }

    tracing::trace!(word = %payload.word, converted = %converted, "autoconvert decision");

    if let Err(err) = apply_last_word_replacement(&payload, &converted) {
        tracing::warn!(error = %err.as_str(), "autoconvert apply failed");
        return;
    }

    update_journal(&payload, &converted);
    crate::input_journal::mark_last_token_autoconverted();
    restore.commit();

    match switch_keyboard_layout() {
        Ok(()) => tracing::trace!("layout switched (autoconvert)"),
        Err(e) => tracing::warn!(error = ?e, "layout switch failed (autoconvert)"),
    }
}

struct AutoconvertGuard;

impl AutoconvertGuard {
    fn try_acquire() -> Result<Self, SkipReason> {
        if AUTOCONVERT_IN_PROGRESS.swap(true, Ordering::AcqRel) {
            return Err(SkipReason::Reentry);
        }
        Ok(Self)
    }
}

impl Drop for AutoconvertGuard {
    fn drop(&mut self) {
        AUTOCONVERT_IN_PROGRESS.store(false, Ordering::Release);
    }
}

struct JournalRestore<'a> {
    payload: &'a LastWordPayload,
    committed: bool,
}

impl<'a> JournalRestore<'a> {
    fn new(payload: &'a LastWordPayload) -> Self {
        Self {
            payload,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for JournalRestore<'_> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        update_journal(self.payload, &self.payload.word);
        tracing::trace!("autoconvert: journal restored");
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum SkipReason {
    Reentry,
    SuffixHasNewline,
    NotAWord,
    NoChangeAfterConvert,
    TooShort,
    ScriptCheckFailed,
    AlreadyCorrect,
    ConvertedConfidenceLow,
    NotBetterEnough,
}

impl SkipReason {
    fn as_str(self) -> &'static str {
        match self {
            SkipReason::Reentry => "reentry",
            SkipReason::SuffixHasNewline => "suffix_has_newline",
            SkipReason::NotAWord => "not_a_word",
            SkipReason::NoChangeAfterConvert => "no_change_after_convert",
            SkipReason::TooShort => "too_short",
            SkipReason::ScriptCheckFailed => "script_check_failed",
            SkipReason::AlreadyCorrect => "already_correct",
            SkipReason::ConvertedConfidenceLow => "converted_confidence_low",
            SkipReason::NotBetterEnough => "not_better_enough",
        }
    }
}

fn has_ascii_vowel(s: &str) -> bool {
    s.chars().any(|ch| {
        let c = ch.to_ascii_lowercase();
        matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
    })
}

fn has_cyrillic_vowel(s: &str) -> bool {
    s.chars().any(|ch| {
        let c = ch.to_lowercase().next().unwrap_or(ch);
        matches!(c, 'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я')
    })
}

fn is_plausible_english_like_token(s: &str) -> bool {
    if !looks_like_ascii_word(s) {
        return false;
    }

    let has_vowel = has_ascii_vowel(s);

    // 'y' intentionally treated as consonant here to reduce false positives.
    let mut consonant_run = 0usize;
    let mut max_consonant_run = 0usize;
    let mut rare = 0usize;

    for ch in s.chars() {
        if ch == '\'' {
            continue;
        }

        let c = ch.to_ascii_lowercase();
        let is_vowel = matches!(c, 'a' | 'e' | 'i' | 'o' | 'u');

        if is_vowel {
            consonant_run = 0;
        } else {
            consonant_run += 1;
            max_consonant_run = max_consonant_run.max(consonant_run);

            if matches!(c, 'j' | 'q' | 'x' | 'z') {
                rare += 1;
            }
        }
    }

    has_vowel && max_consonant_run <= 4 && rare <= 1
}

fn is_plausible_russian_like_token(s: &str) -> bool {
    if !looks_like_cyrillic_word(s) {
        return false;
    }
    if !has_cyrillic_vowel(s) {
        return false;
    }

    let mut consonant_run = 0usize;
    let mut max_consonant_run = 0usize;

    for ch in s.chars() {
        if ch == '\'' || ch == '-' {
            continue;
        }
        if !ch.is_alphabetic() {
            continue;
        }

        let c = ch.to_lowercase().next().unwrap_or(ch);
        let is_vowel = matches!(c, 'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я');

        if is_vowel {
            consonant_run = 0;
        } else {
            consonant_run += 1;
            max_consonant_run = max_consonant_run.max(consonant_run);
        }
    }

    max_consonant_run <= 4
}

#[derive(Copy, Clone, Debug)]
enum ApplyError {
    KeyInjectionFailed,
}

impl ApplyError {
    fn as_str(self) -> &'static str {
        match self {
            ApplyError::KeyInjectionFailed => "key_injection_failed",
        }
    }
}

fn split_trailing_convertible_punct(s: &str) -> (&str, &str) {
    let bytes = s.as_bytes();
    let mut i = bytes.len();

    while i > 0 {
        match bytes[i - 1] {
            b'?' | b'/' | b',' | b'.' => i -= 1,
            _ => break,
        }
    }

    s.split_at(i)
}

fn autoconvert_candidate(p: &LastWordPayload) -> Result<String, SkipReason> {
    ensure_no_newline(p)?;
    ensure_has_letters(&p.word)?;

    let (word_core, word_punct) = split_trailing_convertible_punct(&p.word);

    let converted_core = convert_ru_en_bidirectional(word_core);

    let mut converted = String::with_capacity(converted_core.len() + word_punct.len());
    converted.push_str(&converted_core);
    converted.push_str(word_punct);

    ensure_changed(&p.word, &converted)?;

    Ok(converted)
}

fn language_detector() -> &'static lingua::LanguageDetector {
    use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};

    static DETECTOR: OnceLock<LanguageDetector> = OnceLock::new();
    DETECTOR.get_or_init(|| {
        LanguageDetectorBuilder::from_languages(&[Language::English, Language::Russian])
            .with_minimum_relative_distance(0.20)
            .build()
    })
}

fn looks_like_ascii_word(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    let is_ascii_letter = |b: u8| b.is_ascii_uppercase() || b.is_ascii_lowercase();

    let mut has_letter = false;

    for i in 0..bytes.len() {
        let b = bytes[i];

        if is_ascii_letter(b) {
            has_letter = true;
            continue;
        }

        if b == b'\'' {
            continue;
        }

        // Allow dot or comma only when it is between ASCII letters.
        if (b == b'.' || b == b',')
            && i > 0
            && i + 1 < bytes.len()
            && is_ascii_letter(bytes[i - 1])
            && is_ascii_letter(bytes[i + 1])
        {
            continue;
        }

        return false;
    }

    has_letter
}

fn should_autoconvert_word(
    detector: &lingua::LanguageDetector,
    word: &str,
    converted: &str,
) -> Result<(), SkipReason> {
    use lingua::Language;

    let (word_core, _word_punct) = split_trailing_convertible_punct(word);
    let (conv_core, _conv_punct) = split_trailing_convertible_punct(converted);

    if word_core.chars().count() < MIN_WORD_LEN {
        return Err(SkipReason::TooShort);
    }

    let w_is_ascii = looks_like_ascii_word(word_core);
    let w_is_cyr = looks_like_cyrillic_word(word_core);
    let c_is_ascii = looks_like_ascii_word(conv_core);
    let c_is_cyr = looks_like_cyrillic_word(conv_core);

    if !(w_is_ascii || w_is_cyr) || !(c_is_ascii || c_is_cyr) {
        return Err(SkipReason::ScriptCheckFailed);
    }

    let w_ru = confidence(detector, word_core, Language::Russian);
    let w_en = confidence(detector, word_core, Language::English);
    let c_ru = confidence(detector, conv_core, Language::Russian);
    let c_en = confidence(detector, conv_core, Language::English);

    if w_is_ascii && is_plausible_english_like_token(word_core) {
        return Err(SkipReason::AlreadyCorrect);
    }
    if w_is_cyr && is_plausible_russian_like_token(word_core) {
        return Err(SkipReason::AlreadyCorrect);
    }

    let w_best = w_ru.max(w_en);
    let c_best = c_ru.max(c_en);

    let target = if w_is_ascii {
        Language::Russian
    } else {
        Language::English
    };

    let (w_in_target, c_in_target) = match target {
        Language::Russian => (w_ru, c_ru),
        Language::English => (w_en, c_en),
        _ => unreachable!("only ru/en here"),
    };

    if c_best < MIN_CONVERTED_CONFIDENCE {
        return Err(SkipReason::ConvertedConfidenceLow);
    }

    let min_abs = if w_best < 0.30 {
        0.55
    } else {
        MIN_CONVERTED_CONFIDENCE
    };
    if c_in_target < min_abs {
        return Err(SkipReason::ConvertedConfidenceLow);
    }

    if c_in_target - w_in_target < MIN_CONFIDENCE_GAIN {
        return Err(SkipReason::NotBetterEnough);
    }

    Ok(())
}

fn confidence(detector: &lingua::LanguageDetector, text: &str, lang: lingua::Language) -> f64 {
    detector
        .compute_language_confidence_values(text)
        .iter()
        .find(|(l, _)| *l == lang)
        .map(|(_, v)| *v)
        .unwrap_or(0.0)
}

fn ensure_no_newline(p: &LastWordPayload) -> Result<(), SkipReason> {
    if p.suffix_has_newline {
        return Err(SkipReason::SuffixHasNewline);
    }
    Ok(())
}

fn ensure_has_letters(word: &str) -> Result<(), SkipReason> {
    if word.chars().any(|c| c.is_alphabetic()) {
        return Ok(());
    }
    Err(SkipReason::NotAWord)
}

fn ensure_changed(word: &str, converted: &str) -> Result<(), SkipReason> {
    if word != converted {
        return Ok(());
    }
    Err(SkipReason::NoChangeAfterConvert)
}

fn looks_like_cyrillic_word(s: &str) -> bool {
    let mut has_alpha = false;

    for ch in s.chars() {
        if ch.is_alphabetic() {
            if !is_cyrillic(ch) {
                return false;
            }
            has_alpha = true;
            continue;
        }

        if ch == '\'' || ch == '-' {
            continue;
        }

        return false;
    }

    has_alpha
}

fn is_cyrillic(ch: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&ch) || ('\u{0500}'..='\u{052F}').contains(&ch)
}

fn apply_last_word_replacement(p: &LastWordPayload, converted: &str) -> Result<(), ApplyError> {
    let mut seq = KeySequence::new();
    if apply_last_word_conversion(&mut seq, p, converted) {
        return Ok(());
    }
    Err(ApplyError::KeyInjectionFailed)
}

#[tracing::instrument(level = "trace", skip(state))]
fn convert_last_word_impl(state: &mut AppState, switch_layout: bool) {
    if !foreground_window_alive() {
        tracing::warn!("foreground window is null");
        return;
    }

    if switch_layout && !wait_shift_released(150) {
        tracing::info!("wait_shift_released returned false");
        return;
    }

    sleep_before_convert(state);

    let Some(payload) = take_last_word_payload() else {
        tracing::info!("journal: no last word");
        return;
    };

    let mut restore = JournalRestore::new(&payload);

    if payload.suffix_has_newline {
        tracing::trace!("suffix contains newline, skipping convert_last_word");
        return;
    }

    let converted = convert_ru_en_bidirectional(&payload.word);
    tracing::trace!(%converted, "converted");

    if let Err(err) = apply_last_word_replacement(&payload, &converted) {
        tracing::warn!(error = %err.as_str(), "convert apply failed");
        return;
    }

    update_journal(&payload, &converted);
    restore.commit();

    if switch_layout {
        match switch_keyboard_layout() {
            Ok(()) => tracing::trace!("layout switched"),
            Err(e) => tracing::warn!(error = ?e, "layout switch failed"),
        }
    }
}

fn foreground_window_alive() -> bool {
    let fg = unsafe { GetForegroundWindow() };
    !fg.0.is_null()
}

fn sleep_before_convert(state: &AppState) {
    let delay_ms = crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100);
    tracing::trace!(delay_ms, "sleep before convert");
    thread::sleep(Duration::from_millis(delay_ms as u64));
}

struct LastWordPayload {
    word: String,
    suffix: String,
    word_len: usize,
    suffix_len: usize,
    suffix_spaces_only: bool,
    suffix_has_newline: bool,
}

fn normalize_last_word_payload(mut word: String, mut suffix: String) -> Option<LastWordPayload> {
    fn is_convertible_trailing_punct(ch: char) -> bool {
        matches!(ch, '?' | '/' | ',' | '.')
    }

    if word.is_empty() {
        return None;
    }

    let suffix_has_newline = suffix.contains('\n') || suffix.contains('\r');

    let (first, rest) = match suffix.chars().next() {
        Some(ch) if is_convertible_trailing_punct(ch) => {
            let ch_len = ch.len_utf8();
            (Some(ch), &suffix[ch_len..])
        }
        _ => (None, ""),
    };

    if let Some(ch) = first
        && !suffix_has_newline
        && rest.chars().all(|c| c == ' ' || c == '\t')
    {
        word.push(ch);
        suffix = rest.to_string();
    }

    let word_len = word.chars().count();
    let suffix_len = suffix.chars().count();
    let suffix_spaces_only = !suffix.is_empty() && suffix.chars().all(|c| c == ' ' || c == '\t');

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
}

fn take_last_word_payload() -> Option<LastWordPayload> {
    crate::input_journal::take_last_word_with_suffix()
        .and_then(|(word, suffix)| normalize_last_word_payload(word, suffix))
}

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

fn journal_push_plan<'a>(p: &'a LastWordPayload, converted: &'a str) -> [&'a str; 2] {
    // [0] всегда пишем converted
    // [1] пишем suffix или пустую строку, чтобы не аллоцировать Vec
    [converted, p.suffix.as_str()]
}

/// Updates the input journal to match what was inserted.
fn update_journal(p: &LastWordPayload, converted: &str) {
    let [head, tail] = journal_push_plan(p, converted);

    crate::input_journal::push_text(head);
    if !tail.is_empty() {
        crate::input_journal::push_text(tail);
    }

    tracing::trace!("journal updated");
}

fn delete_with_backspace(seq: &mut KeySequence, count: usize) -> bool {
    repeat_tap(seq, VK_BACKSPACE_KEY, count, "backspace tap failed")
}

fn move_caret_left(seq: &mut KeySequence, count: usize) -> bool {
    repeat_tap(seq, VK_LEFT_KEY, count, "left arrow tap failed")
}

fn move_caret_right(seq: &mut KeySequence, count: usize) -> bool {
    repeat_tap(seq, VK_RIGHT_KEY, count, "right arrow tap failed")
}

fn repeat_tap(seq: &mut KeySequence, vk: VIRTUAL_KEY, count: usize, err_msg: &'static str) -> bool {
    (0..count)
        .try_for_each(|i| seq.tap(vk).then_some(()).ok_or(i))
        .map(|_| true)
        .unwrap_or_else(|i| {
            tracing::error!(i, count, %err_msg);
            false
        })
}

#[cfg(test)]
mod tests {
    use lingua::{Language, LanguageDetectorBuilder};

    use super::*;

    fn detector_ru_en() -> lingua::LanguageDetector {
        LanguageDetectorBuilder::from_languages(&[Language::Russian, Language::English])
            .with_minimum_relative_distance(0.20)
            .build()
    }

    #[test]
    fn autoconvert_does_not_touch_correct_russian_word() {
        let detector = detector_ru_en();

        let word = "привет";
        let converted = convert_ru_en_bidirectional(word);
        assert_eq!(converted, "ghbdtn");

        let decision = should_autoconvert_word(&detector, word, &converted);
        assert!(
            decision.is_err(),
            "should not autoconvert correct Russian word"
        );
    }

    #[test]
    fn autoconvert_converts_mistyped_russian_layout_word() {
        let detector = detector_ru_en();

        let word = "ghbdtn";
        let converted = convert_ru_en_bidirectional(word);
        assert_eq!(converted, "привет");

        match should_autoconvert_word(&detector, word, &converted) {
            Ok(()) => {}
            Err(reason) => {
                panic!(
                    "should autoconvert mistyped Russian layout word, got Err({:?})",
                    reason
                );
            }
        }
    }

    #[test]
    fn autoconvert_does_not_touch_correct_english_ascii_word() {
        let detector = detector_ru_en();

        let word = "world";
        let converted = convert_ru_en_bidirectional(word);
        assert_ne!(converted, word);

        let decision = should_autoconvert_word(&detector, word, &converted);
        assert!(
            decision.is_err(),
            "should not autoconvert correct English ASCII word"
        );
    }

    #[test]
    fn autoconvert_skips_too_short_words() {
        let detector = detector_ru_en();

        let word = "rfr"; // "как"
        let converted = convert_ru_en_bidirectional(word);
        assert_eq!(converted, "как");

        let decision = should_autoconvert_word(&detector, word, &converted);
        assert!(
            decision.is_err(),
            "must skip short words to avoid false positives"
        );
    }

    #[test]
    fn autoconvert_skips_mixed_or_nonword_tokens() {
        let detector = detector_ru_en();

        let word = ";tklf"; // starts with punctuation, should fail script heuristics
        let converted = convert_ru_en_bidirectional(word);
        assert_ne!(converted, word);

        let decision = should_autoconvert_word(&detector, word, &converted);
        assert!(decision.is_err(), "must skip nonword tokens");
    }

    #[test]
    fn normalize_moves_convertible_punct_into_word_when_suffix_is_whitespace() {
        let p = normalize_last_word_payload("ghbdtn".to_string(), "? ".to_string()).unwrap();
        assert_eq!(p.word, "ghbdtn?");
        assert_eq!(p.suffix, " ");
        assert!(p.suffix_spaces_only);
        assert!(!p.suffix_has_newline);
    }

    #[test]
    fn normalize_does_not_move_punct_when_suffix_has_newline() {
        let p = normalize_last_word_payload("ghbdtn".to_string(), "?\n".to_string()).unwrap();
        assert_eq!(p.word, "ghbdtn");
        assert_eq!(p.suffix, "?\n");
        assert!(p.suffix_has_newline);
    }

    #[test]
    fn normalize_does_not_move_punct_when_suffix_has_nonspace_tail() {
        let p = normalize_last_word_payload("ghbdtn".to_string(), "?x".to_string()).unwrap();
        assert_eq!(p.word, "ghbdtn");
        assert_eq!(p.suffix, "?x");
    }

    #[test]
    fn autoconvert_skips_english_typo_like_hellp() {
        let detector = detector_ru_en();

        let word = "hellp";
        let converted = convert_ru_en_bidirectional(word);
        assert_ne!(converted, word);

        let decision = should_autoconvert_word(&detector, word, &converted);
        assert!(decision.is_err(), "must skip english-looking token: {word}");
    }

    #[test]
    fn autoconvert_converts_token_from_reported_sequence_hjyxnmyuj() {
        let detector = detector_ru_en();

        let word = "hjyxnmyuj";
        let converted = convert_ru_en_bidirectional(word);
        assert_eq!(converted, "рончтьнго");

        match should_autoconvert_word(&detector, word, &converted) {
            Ok(()) => {}
            Err(reason) => {
                panic!(
                    "must autoconvert token from reported sequence: {} -> {}, got Err({:?})",
                    word, converted, reason
                );
            }
        }
    }
    #[test]
    fn normalize_does_not_move_nonconvertible_punct() {
        let p = normalize_last_word_payload("ghbdtn".to_string(), "! ".to_string()).unwrap();
        assert_eq!(p.word, "ghbdtn");
        assert_eq!(p.suffix, "! ");
    }

    #[test]
    fn autoconvert_regression_reported_sequence_batch() {
        let detector = detector_ru_en();

        // (token, expected_autoconvert)
        let cases = [
            ("ghbdtn", true),    // привет
            ("hjyxnmyuj", true), // рончтьнго (из репорта)
            ("gjyznyj", true),   // понятно
            ("fdujlyj", true), // "авгодно" (ошибка/мусор, но автоконверт должен срабатывать по текущей цели)
            ("hellp", false),  // английскоподобная опечатка, не трогать
            ("world", false),  // корректное английское слово, не трогать
            ("привет", false), // корректное русское слово, не трогать
        ];

        for (word, should_convert) in cases {
            let converted = convert_ru_en_bidirectional(word);

            let decision = should_autoconvert_word(&detector, word, &converted);
            match (should_convert, decision) {
                (true, Ok(())) => {}
                (false, Err(_)) => {}
                (true, Err(reason)) => {
                    panic!(
                        "expected autoconvert for token: {} -> {}, got Err({:?})",
                        word, converted, reason
                    );
                }
                (false, Ok(())) => {
                    panic!(
                        "expected skip for token: {} -> {}, got Ok(())",
                        word, converted
                    );
                }
            }
        }
    }
    #[test]
    fn normalize_plus_decision_converts_with_trailing_punct_and_space_suffix() {
        let detector = detector_ru_en();

        let p = normalize_last_word_payload("ghbdtn".to_string(), ",   \t".to_string()).unwrap();

        assert_eq!(p.word.as_str(), "ghbdtn,");
        assert_eq!(p.suffix.as_str(), "   \t");

        let converted = convert_ru_en_bidirectional(&p.word);
        let decision = should_autoconvert_word(&detector, &p.word, &converted);

        assert!(
            decision.is_ok(),
            "expected autoconvert decision for normalized token: {} -> {}, got {:?}",
            p.word,
            converted,
            decision
        );
    }
}
