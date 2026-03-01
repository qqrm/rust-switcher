// File: src/domain/text/mapping.rs

/// Direction of text conversion between Russian ЙЦУКЕН and English QWERTY layouts.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ConversionDirection {
    RuToEn,
    EnToRu,
}

const fn is_latin_letter(ch: char) -> bool {
    ch.is_ascii_alphabetic()
}

const fn is_cyrillic_letter(ch: char) -> bool {
    matches!(ch, 'А'..='Я' | 'а'..='я' | 'Ё' | 'ё')
}

const fn map_ru_to_en(ch: char) -> char {
    match ch {
        // punctuation rules (for . , ? keys)
        ',' => '?',
        '.' => '/',
        '?' => '&',

        // number row shift symbols (RU -> EN, same physical keys)
        '"' => '@', // RU Shift+2 -> EN Shift+2
        '№' => '#', // RU Shift+3 -> EN Shift+3
        ';' => '$', // RU Shift+4 -> EN Shift+4
        ':' => '^', // RU Shift+6 -> EN Shift+6
        // RU Shift+7 is '?' -> '&' already covered above
        'й' => 'q',
        'ц' => 'w',
        'у' => 'e',
        'к' => 'r',
        'е' => 't',
        'н' => 'y',
        'г' => 'u',
        'ш' => 'i',
        'щ' => 'o',
        'з' => 'p',
        'х' => '[',
        'ъ' => ']',
        'ф' => 'a',
        'ы' => 's',
        'в' => 'd',
        'а' => 'f',
        'п' => 'g',
        'р' => 'h',
        'о' => 'j',
        'л' => 'k',
        'д' => 'l',
        'ж' => ';',
        'э' => '\'',
        'я' => 'z',
        'ч' => 'x',
        'с' => 'c',
        'м' => 'v',
        'и' => 'b',
        'т' => 'n',
        'ь' => 'm',
        'б' => ',',
        'ю' => '.',
        'ё' => '`',

        'Й' => 'Q',
        'Ц' => 'W',
        'У' => 'E',
        'К' => 'R',
        'Е' => 'T',
        'Н' => 'Y',
        'Г' => 'U',
        'Ш' => 'I',
        'Щ' => 'O',
        'З' => 'P',
        'Х' => '{',
        'Ъ' => '}',
        'Ф' => 'A',
        'Ы' => 'S',
        'В' => 'D',
        'А' => 'F',
        'П' => 'G',
        'Р' => 'H',
        'О' => 'J',
        'Л' => 'K',
        'Д' => 'L',
        'Ж' => ':',
        'Э' => '"',
        'Я' => 'Z',
        'Ч' => 'X',
        'С' => 'C',
        'М' => 'V',
        'И' => 'B',
        'Т' => 'N',
        'Ь' => 'M',
        'Б' => '<',
        'Ю' => '>',
        'Ё' => '~',
        _ => ch,
    }
}

const fn map_en_to_ru(ch: char) -> char {
    match ch {
        // letters / punctuation keys (EN -> RU)
        'q' => 'й',
        'w' => 'ц',
        'e' => 'у',
        'r' => 'к',
        't' => 'е',
        'y' => 'н',
        'u' => 'г',
        'i' => 'ш',
        'o' => 'щ',
        'p' => 'з',
        '[' => 'х',
        ']' => 'ъ',
        'a' => 'ф',
        's' => 'ы',
        'd' => 'в',
        'f' => 'а',
        'g' => 'п',
        'h' => 'р',
        'j' => 'о',
        'k' => 'л',
        'l' => 'д',
        ';' => 'ж',
        '\'' => 'э',
        'z' => 'я',
        'x' => 'ч',
        'c' => 'с',
        'v' => 'м',
        'b' => 'и',
        'n' => 'т',
        'm' => 'ь',
        ',' => 'б',
        '.' => 'ю',
        '`' => 'ё',

        // punctuation rules (for . , ? keys)
        '?' => ',',
        '/' => '.',
        '&' => '?',

        // number row shift symbols (EN -> RU, same physical keys)
        '@' => '"', // EN Shift+2 -> RU Shift+2
        '#' => '№', // EN Shift+3 -> RU Shift+3
        '$' => ';', // EN Shift+4 -> RU Shift+4
        '^' => ':', // EN Shift+6 -> RU Shift+6
        // EN Shift+7 is '&' -> '?' already covered above

        // shifted letters / punctuation keys (EN -> RU)
        'Q' => 'Й',
        'W' => 'Ц',
        'E' => 'У',
        'R' => 'К',
        'T' => 'Е',
        'Y' => 'Н',
        'U' => 'Г',
        'I' => 'Ш',
        'O' => 'Щ',
        'P' => 'З',
        '{' => 'Х',
        '}' => 'Ъ',
        'A' => 'Ф',
        'S' => 'Ы',
        'D' => 'В',
        'F' => 'А',
        'G' => 'П',
        'H' => 'Р',
        'J' => 'О',
        'K' => 'Л',
        'L' => 'Д',
        ':' => 'Ж',
        '"' => 'Э',
        'Z' => 'Я',
        'X' => 'Ч',
        'C' => 'С',
        'V' => 'М',
        'B' => 'И',
        'N' => 'Т',
        'M' => 'Ь',
        '<' => 'Б',
        '>' => 'Ю',
        '~' => 'Ё',
        _ => ch,
    }
}

fn letter_counts(text: &str) -> (usize, usize) {
    let mut cyr = 0usize;
    let mut lat = 0usize;
    for ch in text.chars() {
        if is_cyrillic_letter(ch) {
            cyr += 1;
        } else if is_latin_letter(ch) {
            lat += 1;
        }
    }
    (cyr, lat)
}

/// Returns a conversion direction based on letter balance.
///
/// If the counts are tied (including zero letters), returns `None`.
#[must_use]
pub fn conversion_direction_for_text(text: &str) -> Option<ConversionDirection> {
    let (cyr, lat) = letter_counts(text);
    match cyr.cmp(&lat) {
        std::cmp::Ordering::Greater => Some(ConversionDirection::RuToEn),
        std::cmp::Ordering::Less => Some(ConversionDirection::EnToRu),
        std::cmp::Ordering::Equal => None,
    }
}

/// Converts text between English QWERTY and Russian ЙЦУКЕН keyboard layouts in the given direction.
#[must_use]
pub fn convert_ru_en_with_direction(text: &str, direction: ConversionDirection) -> String {
    // `text.len()` is in bytes. For En->Ru conversions, the output is commonly UTF-8 Cyrillic
    // (2 bytes per character), so we pre-allocate a bit more to avoid reallocations.
    let mut out = match direction {
        ConversionDirection::RuToEn => String::with_capacity(text.len()),
        ConversionDirection::EnToRu => String::with_capacity(text.len().saturating_mul(2)),
    };
    match direction {
        ConversionDirection::RuToEn => {
            for ch in text.chars() {
                out.push(map_ru_to_en(ch));
            }
        }
        ConversionDirection::EnToRu => {
            for ch in text.chars() {
                out.push(map_en_to_ru(ch));
            }
        }
    }
    out
}

/// Convenience wrapper: auto-detect direction (fallback to `RuToEn` on ties).
/// This is intentionally a normal public API so downstream test crates can use it.
#[must_use]
pub fn convert_ru_en_bidirectional(text: &str) -> String {
    let direction = conversion_direction_for_text(text).unwrap_or(ConversionDirection::RuToEn);
    convert_ru_en_with_direction(text, direction)
}
