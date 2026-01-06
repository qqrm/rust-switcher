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
