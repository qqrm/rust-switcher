#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ConversionDirection {
    RuToEn,
    EnToRu,
}

fn is_cyrillic_letter(ch: char) -> bool {
    matches!(ch, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')
}

fn is_latin_letter(ch: char) -> bool {
    ch.is_ascii_alphabetic()
}

fn map_ru_to_en(ch: char) -> char {
    #[rustfmt::skip]
    match ch {
        'й' => 'q', 'ц' => 'w', 'у' => 'e', 'к' => 'r', 'е' => 't', 'н' => 'y', 'г' => 'u', 'ш' => 'i', 'щ' => 'o', 'з' => 'p',
        'х' => '[', 'ъ' => ']',
        'ф' => 'a', 'ы' => 's', 'в' => 'd', 'а' => 'f', 'п' => 'g', 'р' => 'h', 'о' => 'j', 'л' => 'k', 'д' => 'l',
        'ж' => ';', 'э' => '\'',
        'я' => 'z', 'ч' => 'x', 'с' => 'c', 'м' => 'v', 'и' => 'b', 'т' => 'n', 'ь' => 'm',
        'б' => ',', 'ю' => '.',
        'ё' => '`',

        // punctuation rules you requested
        ',' => '?',
        '.' => '/',
        '?' => '&',

        'Й' => 'Q', 'Ц' => 'W', 'У' => 'E', 'К' => 'R', 'Е' => 'T', 'Н' => 'Y', 'Г' => 'U', 'Ш' => 'I', 'Щ' => 'O', 'З' => 'P',
        'Х' => '{', 'Ъ' => '}',
        'Ф' => 'A', 'Ы' => 'S', 'В' => 'D', 'А' => 'F', 'П' => 'G', 'Р' => 'H', 'О' => 'J', 'Л' => 'K', 'Д' => 'L',
        'Ж' => ':', 'Э' => '"',
        'Я' => 'Z', 'Ч' => 'X', 'С' => 'C', 'М' => 'V', 'И' => 'B', 'Т' => 'N', 'Ь' => 'M',
        'Б' => '<', 'Ю' => '>',
        'Ё' => '~',
        _ => ch,
    }
}

fn map_en_to_ru(ch: char) -> char {
    #[rustfmt::skip]
    match ch {
        'q' => 'й', 'w' => 'ц', 'e' => 'у', 'r' => 'к', 't' => 'е', 'y' => 'н', 'u' => 'г', 'i' => 'ш', 'o' => 'щ', 'p' => 'з',
        '[' => 'х', ']' => 'ъ',
        'a' => 'ф', 's' => 'ы', 'd' => 'в', 'f' => 'а', 'g' => 'п', 'h' => 'р', 'j' => 'о', 'k' => 'л', 'l' => 'д',
        ';' => 'ж', '\'' => 'э',
        'z' => 'я', 'x' => 'ч', 'c' => 'с', 'v' => 'м', 'b' => 'и', 'n' => 'т', 'm' => 'ь',
        ',' => 'б', '.' => 'ю',
        '`' => 'ё',

        // punctuation rules you requested
        '?' => ',',
        '/' => '.',
        '&' => '?',

        'Q' => 'Й', 'W' => 'Ц', 'E' => 'У', 'R' => 'К', 'T' => 'Е', 'Y' => 'Н', 'U' => 'Г', 'I' => 'Ш', 'O' => 'Щ', 'P' => 'З',
        '{' => 'Х', '}' => 'Ъ',
        'A' => 'Ф', 'S' => 'Ы', 'D' => 'В', 'F' => 'А', 'G' => 'П', 'H' => 'Р', 'J' => 'О', 'K' => 'Л', 'L' => 'Д',
        ':' => 'Ж', '"' => 'Э',
        'Z' => 'Я', 'X' => 'Ч', 'C' => 'С', 'V' => 'М', 'B' => 'И', 'N' => 'Т', 'M' => 'Ь',
        '<' => 'Б', '>' => 'Ю',
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
pub fn conversion_direction_for_text(text: &str) -> Option<ConversionDirection> {
    let (cyr, lat) = letter_counts(text);
    match cyr.cmp(&lat) {
        std::cmp::Ordering::Greater => Some(ConversionDirection::RuToEn),
        std::cmp::Ordering::Less => Some(ConversionDirection::EnToRu),
        std::cmp::Ordering::Equal => None,
    }
}

/// Converts text between English QWERTY and Russian ЙЦУКЕН keyboard layouts in the given direction.
///
/// Complexity:
/// - O(n) over Unicode scalar values of the input string.
///
/// Notes:
/// - This is a layout conversion, not a transliteration.
/// - Non ASCII and non Russian letters are preserved as is.
pub fn convert_ru_en_with_direction(text: &str, direction: ConversionDirection) -> String {
    let mut out = String::with_capacity(text.len());
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

/// Converts text between English QWERTY and Russian ЙЦУКЕН keyboard layouts in both directions.
///
/// Behavior:
/// - Counts Latin vs Cyrillic letters to choose a conversion direction.
/// - If the counts are equal, defaults to RU -> EN.
///
/// Mapping coverage:
/// - Letters a-z and A-Z.
/// - Punctuation on the same physical keys: brackets, semicolon, quote, comma, dot, backtick.
/// - Russian letters include ё and Ё.
/// - Curly braces `{}` and quotes `"` are produced from shifted bracket/quote keys, matching typical layouts.
///
/// Complexity:
/// - O(n) over Unicode scalar values of the input string.
///
/// Notes:
/// - This is a layout conversion, not a transliteration.
/// - Non ASCII and non Russian letters are preserved as is.
pub fn convert_ru_en_bidirectional(text: &str) -> String {
    let direction = conversion_direction_for_text(text).unwrap_or(ConversionDirection::RuToEn);
    convert_ru_en_with_direction(text, direction)
}
