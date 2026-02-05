use crate::domain::text::mapping::convert_force;

#[test]
fn linux_roundtrip_all_cases() {
    assert_eq!(convert_force("Linux"), "Дштгч");
    assert_eq!(convert_force("Дштгч"), "Linux");

    assert_eq!(convert_force("linux"), "дштгч");
    assert_eq!(convert_force("дштгч"), "linux");

    assert_eq!(convert_force("LiNuX"), "ДшТгЧ");
    assert_eq!(convert_force("ДшТгЧ"), "LiNuX");
}

#[test]
fn russian_word_roundtrip_lowercase() {
    assert_eq!(convert_force("привет"), "ghbdtn");
    assert_eq!(convert_force("ghbdtn"), "привет");
}

#[test]
fn punctuation_bottom_row_physical_mapping() {
    // EN -> RU (same physical keys)
    assert_eq!(convert_force(".,/"), "юб.");
    assert_eq!(convert_force("<>?"), "БЮ,");
    assert_eq!(convert_force(";:'\""), "жЖэЭ");

    // RU -> EN
    assert_eq!(convert_force("юб."), ".,/");
    assert_eq!(convert_force("БЮ,"), "<>?");
    assert_eq!(convert_force("жЖэЭ"), ";:'\"");
}

#[test]
fn digit_row_shift_symbols_mapping() {
    assert_eq!(convert_force("@#$%^&"), "\"№;:%?");
    assert_eq!(convert_force("\"№;:%?"), "@#$%^&");
}

#[test]
fn involution_on_common_samples() {
    let samples = [
        "Linux",
        "Дштгч",
        "linux",
        "дштгч",
        "ghbdtn!!!",
        "привет???",
        "@#$%^&*()",
        "[]{};:'\",./<>",
        "Тест123 test123",
    ];

    for s in samples {
        assert_eq!(convert_force(&convert_force(s)), s);
    }
}

#[test]
fn ampersand_prefers_latin_direction_when_present() {
    assert_eq!(convert_ru_en_bidirectional("a&"), "ф?");
    assert_eq!(convert_ru_en_bidirectional("я?"), "z&");
}
