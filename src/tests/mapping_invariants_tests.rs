use crate::domain::text::mapping::convert_ru_en_bidirectional;

const LATIN_BIJECTIVE: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,.`QWERTYUIOP{}ASDFGHJKL:\"ZXCVBNM<>~";

const CYRILLIC_BIJECTIVE: &str =
    "йцукенгшщзхъфывапролджэячсмитьбюёЙЦУКЕНГШЩЗХЪФЫВАПРОЛДЖЭЯЧСМИТЬБЮЁ";

fn xorshift64(seed: &mut u64) -> u64 {
    let mut x = *seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *seed = x;
    x
}

fn gen_string(seed: &mut u64, alphabet: &[char], max_len: usize) -> String {
    let len = (xorshift64(seed) as usize) % (max_len + 1);
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        let idx = (xorshift64(seed) as usize) % alphabet.len();
        out.push(alphabet[idx]);
    }
    out
}

#[test]
fn mapping_roundtrip_latin_only_is_identity_on_double_convert() {
    let alphabet: Vec<char> = LATIN_BIJECTIVE.chars().collect();

    let mut seed = 0xD1A5_3EED_5EED_1234u64;
    for _ in 0..2000 {
        let s = gen_string(&mut seed, &alphabet, 64);
        let t = convert_ru_en_bidirectional(&s);
        let u = convert_ru_en_bidirectional(&t);
        assert_eq!(u, s, "latin roundtrip failed: s={s:?} t={t:?} u={u:?}");
    }
}

#[test]
fn mapping_roundtrip_cyrillic_only_is_identity_on_double_convert() {
    let alphabet: Vec<char> = CYRILLIC_BIJECTIVE.chars().collect();

    let mut seed = 0xBADC_0FFE_EE12_3456u64;
    for _ in 0..2000 {
        let s = gen_string(&mut seed, &alphabet, 64);
        let t = convert_ru_en_bidirectional(&s);
        let u = convert_ru_en_bidirectional(&t);
        assert_eq!(u, s, "cyr roundtrip failed: s={s:?} t={t:?} u={u:?}");
    }
}

#[test]
fn punctuation_rules_apply_only_in_en_to_ru_mode() {
    // Lat dominates: en_to_ru, so '/' -> '.' and '?' -> ','
    assert_eq!(convert_ru_en_bidirectional("a/"), "ф.");
    assert_eq!(convert_ru_en_bidirectional("a?"), "ф,");

    // Cyr dominates: ru_to_en, so '/' is not remapped by en_to_ru punctuation rule
    assert_eq!(convert_ru_en_bidirectional("/а"), "/f");
}

#[test]
fn ampersand_prefers_latin_direction_when_present() {
    assert_eq!(convert_ru_en_bidirectional("a&"), "ф?");
    assert_eq!(convert_ru_en_bidirectional("я?"), "z&");
}
