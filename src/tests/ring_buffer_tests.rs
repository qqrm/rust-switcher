use crate::input::ring_buffer;

#[test]
fn take_last_word_with_suffix_basic() {
    ring_buffer::invalidate();
    ring_buffer::push_text("hello world   ");
    let p = ring_buffer::take_last_word_with_suffix().expect("expected payload");
    assert_eq!(p.0, "world");
    assert_eq!(p.1, "   ");
}

#[test]
fn take_last_word_with_suffix_ignores_trailing_whitespace_only() {
    ring_buffer::invalidate();
    ring_buffer::push_text("   \t\n");
    assert!(ring_buffer::take_last_word_with_suffix().is_none());
}

#[test]
fn last_char_triggers_autoconvert_on_first_whitespace_after_word() {
    ring_buffer::invalidate();
    ring_buffer::push_text("abc");
    assert!(!ring_buffer::last_char_triggers_autoconvert());
    ring_buffer::push_text(" ");
    assert!(ring_buffer::last_char_triggers_autoconvert());
}

#[test]
fn last_char_triggers_autoconvert_on_punctuation_after_nonspace() {
    ring_buffer::invalidate();
    ring_buffer::push_text("abc");
    ring_buffer::push_text(".");
    assert!(ring_buffer::last_char_triggers_autoconvert());
}

#[test]
fn mark_last_token_autoconverted_roundtrip() {
    ring_buffer::invalidate();
    ring_buffer::push_text("abc");
    assert!(!ring_buffer::last_token_autoconverted());

    ring_buffer::mark_last_token_autoconverted();
    assert!(ring_buffer::last_token_autoconverted());

    // push_text is used to mirror programmatic typing into the journal,
    // so it must not reset the autoconverted flag.
    ring_buffer::push_text(" ");
    ring_buffer::push_text("1");
    assert!(ring_buffer::last_token_autoconverted());

    // The flag resets when the journal is invalidated.
    ring_buffer::invalidate();
    assert!(!ring_buffer::last_token_autoconverted());
}

#[test]
fn take_last_word_with_suffix_keeps_trailing_punct_in_word_when_suffix_is_spaces() {
    ring_buffer::invalidate();
    ring_buffer::push_text("hello, ");
    let (word, suffix) = ring_buffer::take_last_word_with_suffix().unwrap();
    assert_eq!(word, "hello,");
    assert_eq!(suffix, " ");
}

#[test]
fn take_last_word_with_suffix_splits_on_newline() {
    ring_buffer::invalidate();
    ring_buffer::push_text("hello\nworld ");
    let (word, suffix) = ring_buffer::take_last_word_with_suffix().unwrap();
    assert_eq!(word, "world");
    assert_eq!(suffix, " ");
}

#[test]
fn take_last_word_with_suffix_handles_tabs_and_multiple_spaces() {
    ring_buffer::invalidate();
    ring_buffer::push_text("hello\tworld   ");
    let (word, suffix) = ring_buffer::take_last_word_with_suffix().unwrap();
    assert_eq!(word, "world");
    assert_eq!(suffix, "   ");
}

#[test]
fn take_last_word_with_suffix_single_letter_with_punct() {
    ring_buffer::invalidate();
    ring_buffer::push_text("a. ");
    let (word, suffix) = ring_buffer::take_last_word_with_suffix().unwrap();
    assert_eq!(word, "a.");
    assert_eq!(suffix, " ");
}

#[test]
fn take_last_word_with_suffix_returns_none_when_buffer_empty() {
    ring_buffer::invalidate();
    assert!(ring_buffer::take_last_word_with_suffix().is_none());
}
