use std::sync::OnceLock;

use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};

use super::mapping::{ConversionDirection, convert_ru_en_with_direction};

const MIN_SOURCE_CONFIDENCE: f64 = 0.45;
const MIN_TARGET_CONFIDENCE: f64 = 0.60;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Script {
    Latin,
    Cyrillic,
}

pub(crate) fn text_looks_correct(text: &str) -> bool {
    let tokens = word_tokens(text);
    if tokens.is_empty() {
        return false;
    }

    let Some(source_script) = classify_tokens(&tokens) else {
        return false;
    };
    if !tokens
        .iter()
        .all(|token| token_plausible_for_script(token, source_script))
    {
        return false;
    }

    let detector = language_detector();
    let source_lang = language_for_script(source_script);
    let source_confidence = confidence(detector, text, source_lang);
    if source_confidence < MIN_SOURCE_CONFIDENCE {
        return false;
    }

    let converted = convert_ru_en_with_direction(text, direction_for_script(source_script));
    let converted_tokens = word_tokens(&converted);
    let target_script = opposite_script(source_script);
    let converted_looks_like_target = classify_tokens(&converted_tokens) == Some(target_script)
        && converted_tokens
            .iter()
            .all(|token| token_plausible_for_script(token, target_script))
        && confidence(detector, &converted, language_for_script(target_script)) >= MIN_TARGET_CONFIDENCE;

    !converted_looks_like_target
}

fn language_detector() -> &'static LanguageDetector {
    static DETECTOR: OnceLock<LanguageDetector> = OnceLock::new();
    DETECTOR.get_or_init(|| {
        LanguageDetectorBuilder::from_languages(&[Language::English, Language::Russian])
            .with_minimum_relative_distance(0.20)
            .build()
    })
}

fn word_tokens(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .filter_map(trim_word_token)
        .filter(|token| token.chars().any(char::is_alphabetic))
        .collect()
}

fn trim_word_token(token: &str) -> Option<&str> {
    let start = token
        .char_indices()
        .find_map(|(idx, ch)| is_word_token_char(ch).then_some(idx))?;
    let end = token
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| is_word_token_char(ch).then_some(idx + ch.len_utf8()))?;
    (start < end).then_some(&token[start..end])
}

fn is_word_token_char(ch: char) -> bool {
    ch.is_alphabetic() || ch == '\'' || ch == '-'
}

fn classify_tokens(tokens: &[&str]) -> Option<Script> {
    let mut script = None;
    for token in tokens {
        let token_script = classify_token(token)?;
        if script.is_some_and(|known| known != token_script) {
            return None;
        }
        script = Some(token_script);
    }
    script
}

fn classify_token(token: &str) -> Option<Script> {
    let mut script = None;
    for ch in token.chars().filter(|ch| ch.is_alphabetic()) {
        let ch_script = if ch.is_ascii_alphabetic() {
            Script::Latin
        } else if is_cyrillic(ch) {
            Script::Cyrillic
        } else {
            return None;
        };
        if script.is_some_and(|known| known != ch_script) {
            return None;
        }
        script = Some(ch_script);
    }
    script
}

fn token_plausible_for_script(token: &str, script: Script) -> bool {
    match script {
        Script::Latin => is_plausible_english_like_token(token),
        Script::Cyrillic => is_plausible_russian_like_token(token),
    }
}

fn direction_for_script(script: Script) -> ConversionDirection {
    match script {
        Script::Latin => ConversionDirection::EnToRu,
        Script::Cyrillic => ConversionDirection::RuToEn,
    }
}

fn opposite_script(script: Script) -> Script {
    match script {
        Script::Latin => Script::Cyrillic,
        Script::Cyrillic => Script::Latin,
    }
}

fn language_for_script(script: Script) -> Language {
    match script {
        Script::Latin => Language::English,
        Script::Cyrillic => Language::Russian,
    }
}

fn confidence(detector: &LanguageDetector, text: &str, lang: Language) -> f64 {
    detector
        .compute_language_confidence_values(text)
        .iter()
        .find(|(l, _)| *l == lang)
        .map_or(0.0, |(_, v)| *v)
}

fn is_plausible_english_like_token(token: &str) -> bool {
    let letters: Vec<char> = token
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .map(|ch| ch.to_ascii_lowercase())
        .collect();
    if letters.is_empty() || !letters.iter().any(|ch| matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u')) {
        return false;
    }

    let mut consonant_run = 0usize;
    let mut max_consonant_run = 0usize;
    let mut rare = 0usize;
    for ch in letters {
        if matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u') {
            consonant_run = 0;
        } else {
            consonant_run += 1;
            max_consonant_run = max_consonant_run.max(consonant_run);
            if matches!(ch, 'j' | 'q' | 'x' | 'z') {
                rare += 1;
            }
        }
    }

    max_consonant_run <= 4 && rare <= 1
}

fn is_plausible_russian_like_token(token: &str) -> bool {
    let letters: Vec<char> = token
        .chars()
        .filter(|ch| ch.is_alphabetic())
        .map(|ch| ch.to_lowercase().next().unwrap_or(ch))
        .collect();
    if letters.is_empty() || !letters.iter().all(|ch| is_cyrillic(*ch)) {
        return false;
    }
    if !letters
        .iter()
        .any(|ch| matches!(ch, 'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я'))
    {
        return false;
    }

    let mut consonant_run = 0usize;
    let mut max_consonant_run = 0usize;
    for ch in letters {
        if matches!(ch, 'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я') {
            consonant_run = 0;
        } else {
            consonant_run += 1;
            max_consonant_run = max_consonant_run.max(consonant_run);
        }
    }

    max_consonant_run <= 4
}

fn is_cyrillic(ch: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&ch) || ('\u{0500}'..='\u{052F}').contains(&ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_guard_accepts_correct_russian_phrase() {
        assert!(text_looks_correct("привет как дела"));
    }

    #[test]
    fn smart_guard_rejects_wrong_layout_latin_phrase() {
        assert!(!text_looks_correct("ghbdtn rfr ltkf"));
    }

    #[test]
    fn smart_guard_rejects_wrong_layout_cyrillic_word_with_good_conversion() {
        assert!(!text_looks_correct("руддщ"));
    }

    #[test]
    fn smart_guard_accepts_correct_english_phrase() {
        assert!(text_looks_correct("hello how are you"));
    }
}
