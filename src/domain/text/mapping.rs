#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ConversionDirection {
    RuToEn,
    EnToRu,
}

// Physical key inversion between:
// EN (US QWERTY) and RU (ЙЦУКЕН).
//
// Important: direction is chosen ONCE per token/string, then applied to all chars.
// This is required for punctuation/symbols to be reversible.
const EN_UNSHIFT: &str = "`qwertyuiop[]asdfghjkl;'zxcvbnm,./";
const RU_UNSHIFT: &str = "ёйцукенгшщзхъфывапролджэячсмитьбю.";

const EN_SHIFT: &str = "~QWERTYUIOP{}ASDFGHJKL:\"ZXCVBNM<>?";
const RU_SHIFT: &str = "ЁЙЦУКЕНГШЩЗХЪФЫВАПРОЛДЖЭЯЧСМИТЬБЮ,";

// Digit row (Shift+1..=Shift+=) differs on RU layout.
const EN_DIGIT_SHIFT: &str = "!@#$%^&*()_+";
const RU_DIGIT_SHIFT: &str = "!\"№;%:?*()_+";

fn map_by_table(ch: char, from: &str, to: &str) -> Option<char> {
    // Both strings are ASCII on EN side, RU side is UTF-8; we compare by chars.
    let mut fi = from.chars();
    let mut ti = to.chars();
    loop {
        match (fi.next(), ti.next()) {
            (Some(f), Some(t)) => {
                if f == ch {
                    return Some(t);
                }
            }
            _ => return None,
        }
    }
}

fn map_en_to_ru(ch: char) -> char {
    // order matters: digit-shift first (contains '@', '#', '$', '^', '&')
    if let Some(x) = map_by_table(ch, EN_DIGIT_SHIFT, RU_DIGIT_SHIFT) {
        return x;
    }
    if let Some(x) = map_by_table(ch, EN_SHIFT, RU_SHIFT) {
        return x;
    }
    if let Some(x) = map_by_table(ch, EN_UNSHIFT, RU_UNSHIFT) {
        return x;
    }
    // keys identical / not handled
    ch
}

fn map_ru_to_en(ch: char) -> char {
    if let Some(x) = map_by_table(ch, RU_DIGIT_SHIFT, EN_DIGIT_SHIFT) {
        return x;
    }
    if let Some(x) = map_by_table(ch, RU_SHIFT, EN_SHIFT) {
        return x;
    }
    if let Some(x) = map_by_table(ch, RU_UNSHIFT, EN_UNSHIFT) {
        return x;
    }
    ch
}

fn score_direction(s: &str) -> (usize, usize) {
    let mut en_hits = 0usize;
    let mut ru_hits = 0usize;

    for ch in s.chars() {
        let to_ru = map_en_to_ru(ch);
        let to_en = map_ru_to_en(ch);

        if to_ru != ch {
            en_hits += 1;
        }
        if to_en != ch {
            ru_hits += 1;
        }
    }

    (en_hits, ru_hits)
}

pub fn guess_direction(s: &str) -> ConversionDirection {
    let (en_hits, ru_hits) = score_direction(s);

    if ru_hits > en_hits {
        ConversionDirection::RuToEn
    } else {
        // tie -> default EN->RU (most common: typed on EN but wanted RU)
        ConversionDirection::EnToRu
    }
}

pub fn convert_ru_en_with_direction(input: &str, direction: ConversionDirection) -> String {
    match direction {
        ConversionDirection::RuToEn => input.chars().map(map_ru_to_en).collect(),
        ConversionDirection::EnToRu => input.chars().map(map_en_to_ru).collect(),
    }
}

// “Force” conversion used by manual convert and by autoconvert candidate generation.
// It picks a direction ONCE for the whole string and converts all chars.
pub fn convert_force(input: &str) -> String {
    let dir = guess_direction(input);
    convert_ru_en_with_direction(input, dir)
}
