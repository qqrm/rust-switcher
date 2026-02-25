#![allow(clippy::unwrap_used, clippy::expect_used)]
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

use super::{
    convert::expected_direction_for_foreground_window,
    mapping::{ConversionDirection, conversion_direction_for_text, convert_ru_en_with_direction},
    switch_keyboard_layout, wait_shift_released,
};
use crate::{
    app::AppState,
    conversion::input::{KeySequence, send_text_unicode},
    input_journal::{InputRun, LayoutTag, RunKind, RunOrigin},
};
const VK_BACKSPACE_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x08);
const VK_LEFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x25);
const VK_RIGHT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x27);
const MIN_WORD_LEN: usize = 4;
const MIN_CONVERTED_CONFIDENCE: f64 = 0.70;
const MIN_CONFIDENCE_GAIN: f64 = 0.25;
static AUTOCONVERT_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
fn convert_with_layout_fallback(text: &str, layout: &LayoutTag) -> String {
    let direction = match layout {
        LayoutTag::Ru => Some(ConversionDirection::RuToEn),
        LayoutTag::En => Some(ConversionDirection::EnToRu),
        LayoutTag::Other(_) | LayoutTag::Unknown => {
            conversion_direction_for_text(text).or_else(expected_direction_for_foreground_window)
        }
    }
    .unwrap_or(ConversionDirection::RuToEn);
    convert_ru_en_with_direction(text, direction)
}
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
    if let Err(reason) = should_autoconvert_word(detector, &payload.run.text, &converted) {
        tracing::trace!(reason = %reason.as_str(), "autoconvert skip: decision");
        return;
    }
    tracing::trace!(word = %payload.run.text, converted = %converted, "autoconvert decision");
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
    payload: &'a LastRunPayload,
    committed: bool,
}
impl<'a> JournalRestore<'a> {
    fn new(payload: &'a LastRunPayload) -> Self {
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
        restore_journal_original(self.payload);
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
fn autoconvert_candidate(p: &LastRunPayload) -> Result<String, SkipReason> {
    ensure_no_newline(p)?;
    ensure_has_letters(&p.run.text)?;
    let converted = convert_with_layout_fallback(&p.run.text, &p.run.layout);
    ensure_changed(&p.run.text, &converted)?;
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
fn trailing_convertible_punct_count(s: &str) -> usize {
    s.chars()
        .rev()
        .take_while(|ch| matches!(ch, '?' | '/' | ',' | '.'))
        .count()
}
fn trim_tail_chars(s: &str, n: usize) -> &str {
    if n == 0 {
        return s;
    }
    let total = s.chars().count();
    if n >= total {
        return "";
    }
    let keep = total - n;
    let byte_idx = s.char_indices().nth(keep).map_or(s.len(), |(i, _)| i);
    &s[..byte_idx]
}
fn should_autoconvert_word(
    detector: &lingua::LanguageDetector,
    word: &str,
    converted: &str,
) -> Result<(), SkipReason> {
    use lingua::Language;
    const MIN_CONVERTED_EN_CONF_FOR_OVERRIDE: f64 = 0.80;
    let trailing_punct = trailing_convertible_punct_count(word);
    let word_analysis = trim_tail_chars(word, trailing_punct);
    let conv_analysis = trim_tail_chars(converted, trailing_punct);
    if word_analysis.is_empty() || conv_analysis.is_empty() {
        return Err(SkipReason::ScriptCheckFailed);
    }
    if word_analysis.chars().count() < MIN_WORD_LEN {
        return Err(SkipReason::TooShort);
    }
    let w_is_ascii = looks_like_ascii_word(word_analysis);
    let w_is_cyr = looks_like_cyrillic_word(word_analysis);
    let c_is_ascii = looks_like_ascii_word(conv_analysis);
    let c_is_cyr = looks_like_cyrillic_word(conv_analysis);
    if !(w_is_ascii || w_is_cyr) || !(c_is_ascii || c_is_cyr) {
        return Err(SkipReason::ScriptCheckFailed);
    }
    let w_ru = confidence(detector, word_analysis, Language::Russian);
    let w_en = confidence(detector, word_analysis, Language::English);
    let c_ru = confidence(detector, conv_analysis, Language::Russian);
    let c_en = confidence(detector, conv_analysis, Language::English);
    // Keep the English guard: do not convert real English words to Russian.
    if w_is_ascii && is_plausible_english_like_token(word_analysis) {
        return Err(SkipReason::AlreadyCorrect);
    }
    // Russian guard is conditional: if conversion yields a strong English candidate, do not short circuit.
    if w_is_cyr && is_plausible_russian_like_token(word_analysis) {
        let converted_looks_english = is_plausible_english_like_token(conv_analysis)
            && c_en >= MIN_CONVERTED_EN_CONF_FOR_OVERRIDE;
        if !converted_looks_english {
            return Err(SkipReason::AlreadyCorrect);
        }
    }
    let w_best = w_ru.max(w_en);
    let c_best = c_ru.max(c_en);
    let target = if w_is_ascii {
        Language::Russian
    } else {
        Language::English
    };
    let (w_in_target, c_in_target) = if matches!(target, Language::Russian) {
        (w_ru, c_ru)
    } else {
        (w_en, c_en)
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
        .map_or(0.0, |(_, v)| *v)
}
fn ensure_no_newline(p: &LastRunPayload) -> Result<(), SkipReason> {
    if p.suffix_has_newline {
        return Err(SkipReason::SuffixHasNewline);
    }
    Ok(())
}
fn ensure_has_letters(word: &str) -> Result<(), SkipReason> {
    if word.chars().any(char::is_alphabetic) {
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
fn apply_last_word_replacement(p: &LastRunPayload, converted: &str) -> Result<(), ApplyError> {
    if apply_last_word_conversion(p, converted) {
        Ok(())
    } else {
        Err(ApplyError::KeyInjectionFailed)
    }
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
    let converted = convert_with_layout_fallback(&payload.run.text, &payload.run.layout);
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
    thread::sleep(Duration::from_millis(u64::from(delay_ms)));
}
struct LastRunPayload {
    run: InputRun,
    suffix_runs: Vec<InputRun>,
    suffix_text: String,
    run_len: usize,
    suffix_len: usize,
    suffix_spaces_only: bool,
    suffix_has_newline: bool,
}
fn join_suffix_text(suffix_runs: &[InputRun]) -> String {
    suffix_runs.iter().map(|run| run.text.as_str()).collect()
}
fn take_last_word_payload() -> Option<LastRunPayload> {
    let (run, suffix_runs) = crate::input_journal::take_last_layout_run_with_suffix()?;
    if run.kind != RunKind::Text || run.text.is_empty() {
        return None;
    }
    let suffix_text = join_suffix_text(&suffix_runs);
    let run_len = run.text.chars().count();
    let suffix_len = suffix_text.chars().count();
    let suffix_spaces_only =
        !suffix_text.is_empty() && suffix_text.chars().all(|c| c == ' ' || c == '\t');
    let suffix_has_newline = suffix_text.contains('\n') || suffix_text.contains('\r');
    tracing::trace!(
        run_text = %run.text,
        run_layout = ?run.layout,
        run_origin = ?run.origin,
        suffix_text = %suffix_text,
        run_len,
        suffix_len,
        suffix_spaces_only,
        suffix_has_newline,
        "journal extracted run"
    );
    Some(LastRunPayload {
        run,
        suffix_runs,
        suffix_text,
        run_len,
        suffix_len,
        suffix_spaces_only,
        suffix_has_newline,
    })
}
fn apply_last_word_conversion(p: &LastRunPayload, converted: &str) -> bool {
    const MAX_TAPS: usize = 4096;
    let run_len = p.run_len.min(MAX_TAPS);
    let suffix_len = p.suffix_len.min(MAX_TAPS);
    if p.suffix_spaces_only {
        move_caret_left(suffix_len)
            && delete_with_backspace(run_len)
            && send_text_unicode(converted)
            && move_caret_right(suffix_len)
    } else {
        let delete_count = p.run_len.saturating_add(p.suffix_len).min(MAX_TAPS);
        delete_with_backspace(delete_count)
            && send_text_unicode(converted)
            && (p.suffix_text.is_empty() || send_text_unicode(&p.suffix_text))
    }
}
fn flipped_layout(layout: &LayoutTag) -> LayoutTag {
    match layout {
        LayoutTag::Ru => LayoutTag::En,
        LayoutTag::En => LayoutTag::Ru,
        LayoutTag::Other(id) => LayoutTag::Other(*id),
        LayoutTag::Unknown => LayoutTag::Unknown,
    }
}
/// Updates the input journal to match what was inserted.
fn restore_journal_original(p: &LastRunPayload) {
    crate::input_journal::push_run(p.run.clone());
    crate::input_journal::push_runs(p.suffix_runs.iter().cloned());
    tracing::trace!("journal restored (original metadata)");
}
fn update_journal(p: &LastRunPayload, converted: &str) {
    crate::input_journal::push_run(InputRun {
        text: converted.to_string(),
        layout: flipped_layout(&p.run.layout),
        origin: RunOrigin::Programmatic,
        kind: RunKind::Text,
    });
    crate::input_journal::push_runs(p.suffix_runs.iter().cloned());
    tracing::trace!("journal updated");
}
fn delete_with_backspace(count: usize) -> bool {
    repeat_tap(VK_BACKSPACE_KEY, count, "backspace tap failed")
}
fn move_caret_left(count: usize) -> bool {
    repeat_tap(VK_LEFT_KEY, count, "left arrow tap failed")
}
fn move_caret_right(count: usize) -> bool {
    repeat_tap(VK_RIGHT_KEY, count, "right arrow tap failed")
}
fn repeat_tap(vk: VIRTUAL_KEY, count: usize, err_msg: &'static str) -> bool {
    match (0..count).try_for_each(|i| KeySequence::tap(vk).then_some(()).ok_or(i)) {
        Ok(()) => true,
        Err(i) => {
            tracing::error!(i, count, %err_msg);
            false
        }
    }
}
#[cfg(test)]
mod tests {
    use lingua::{Language, LanguageDetectorBuilder};

    use super::*;
    use crate::input::ring_buffer;
    fn detector_ru_en() -> lingua::LanguageDetector {
        LanguageDetectorBuilder::from_languages(&[Language::Russian, Language::English])
            .with_minimum_relative_distance(0.20)
            .build()
    }
    #[test]
    fn ru_layout_punctuation_run_converts_ru_to_en() {
        assert_eq!(convert_with_layout_fallback(",.", &LayoutTag::Ru), "?/");
    }
    #[test]
    fn en_layout_punctuation_run_converts_en_to_ru() {
        assert_eq!(convert_with_layout_fallback(",.", &LayoutTag::En), "бю");
    }
    #[test]
    fn known_layout_overrides_text_heuristic() {
        // Mixed punctuation has no letter heuristic signal, but known layout enforces direction.
        assert_eq!(convert_with_layout_fallback(".", &LayoutTag::Ru), "/");
        assert_eq!(convert_with_layout_fallback(".", &LayoutTag::En), "ю");
    }
    #[test]
    fn update_and_restore_preserve_run_metadata() {
        ring_buffer::invalidate();
        ring_buffer::push_runs([
            InputRun {
                text: "abc,".to_string(),
                layout: LayoutTag::En,
                origin: RunOrigin::Physical,
                kind: RunKind::Text,
            },
            InputRun {
                text: "  ".to_string(),
                layout: LayoutTag::En,
                origin: RunOrigin::Physical,
                kind: RunKind::Whitespace,
            },
        ]);
        let payload = take_last_word_payload().expect("payload expected");
        update_journal(&payload, "фисб");
        let (run, suffix) = ring_buffer::take_last_layout_run_with_suffix().expect("run expected");
        assert_eq!(run.layout, LayoutTag::Ru);
        assert_eq!(run.origin, RunOrigin::Programmatic);
        assert_eq!(run.kind, RunKind::Text);
        assert_eq!(run.text, "фисб");
        assert_eq!(suffix.len(), 1);
        assert_eq!(suffix[0].layout, LayoutTag::En);
        assert_eq!(suffix[0].origin, RunOrigin::Physical);
        assert_eq!(suffix[0].kind, RunKind::Whitespace);
        ring_buffer::push_run(payload.run.clone());
        ring_buffer::push_runs(payload.suffix_runs.clone());
        let (restored, restored_suffix) =
            ring_buffer::take_last_layout_run_with_suffix().expect("restored payload expected");
        assert_eq!(restored, payload.run);
        assert_eq!(restored_suffix, payload.suffix_runs);
    }
    #[test]
    fn convert_candidate_converts_entire_run_without_punct_peel() {
        let payload = LastRunPayload {
            run: InputRun {
                text: "ghbdtn,".to_string(),
                layout: LayoutTag::En,
                origin: RunOrigin::Physical,
                kind: RunKind::Text,
            },
            suffix_runs: Vec::new(),
            suffix_text: String::new(),
            run_len: 7,
            suffix_len: 0,
            suffix_spaces_only: false,
            suffix_has_newline: false,
        };
        let converted = autoconvert_candidate(&payload).expect("candidate should convert");
        assert_eq!(converted, "приветб");
    }
    #[test]
    fn autoconvert_does_not_touch_correct_russian_word() {
        let detector = detector_ru_en();
        let word = "привет";
        let converted = convert_with_layout_fallback(word, &LayoutTag::Ru);
        assert_eq!(converted, "ghbdtn");
        let decision = should_autoconvert_word(&detector, word, &converted);
        assert!(
            decision.is_err(),
            "should not autoconvert correct Russian word"
        );
    }
    #[test]
    fn journal_restore_drop_restores_original_metadata() {
        ring_buffer::invalidate();
        ring_buffer::push_runs([
            InputRun {
                text: "abc,".to_string(),
                layout: LayoutTag::En,
                origin: RunOrigin::Physical,
                kind: RunKind::Text,
            },
            InputRun {
                text: "  ".to_string(),
                layout: LayoutTag::En,
                origin: RunOrigin::Physical,
                kind: RunKind::Whitespace,
            },
        ]);
        let payload = take_last_word_payload().expect("payload expected");
        {
            let _restore = JournalRestore::new(&payload);
        }
        let (restored, restored_suffix) =
            ring_buffer::take_last_layout_run_with_suffix().expect("restored payload expected");
        assert_eq!(restored, payload.run);
        assert_eq!(restored_suffix, payload.suffix_runs);
    }
    #[test]
    fn autoconvert_decision_accepts_trailing_convertible_punctuation() {
        let detector = detector_ru_en();
        let word = "ghbdtn,";
        let converted = convert_with_layout_fallback(word, &LayoutTag::En);
        assert_eq!(converted, "приветб");
        assert!(should_autoconvert_word(&detector, word, &converted).is_ok());
    }
    #[test]
    fn suffix_spaces_only_is_false_for_newline_suffix() {
        ring_buffer::invalidate();
        ring_buffer::push_runs([
            InputRun {
                text: "ghbdtn".to_string(),
                layout: LayoutTag::En,
                origin: RunOrigin::Physical,
                kind: RunKind::Text,
            },
            InputRun {
                text: "\n".to_string(),
                layout: LayoutTag::En,
                origin: RunOrigin::Physical,
                kind: RunKind::Whitespace,
            },
        ]);
        let payload = take_last_word_payload().expect("payload expected");
        assert!(payload.suffix_has_newline);
        assert!(!payload.suffix_spaces_only);
    }
    #[test]
    fn autoconvert_converts_mistyped_russian_layout_word() {
        let detector = detector_ru_en();
        let word = "ghbdtn";
        let converted = convert_with_layout_fallback(word, &LayoutTag::En);
        assert_eq!(converted, "привет");
        match should_autoconvert_word(&detector, word, &converted) {
            Ok(()) => {}
            Err(reason) => {
                panic!("should autoconvert mistyped Russian layout word, got Err({reason:?})");
            }
        }
    }
}
