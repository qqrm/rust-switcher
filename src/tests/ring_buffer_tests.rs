use crate::input::ring_buffer::{self, InputRun, LayoutTag, RunKind, RunOrigin};

#[test]
fn run_journal_merges_contiguous_same_metadata() {
    ring_buffer::invalidate();
    ring_buffer::push_run(InputRun {
        text: "ab".to_string(),
        layout: LayoutTag::En,
        origin: RunOrigin::Physical,
        kind: RunKind::Text,
    });
    ring_buffer::push_run(InputRun {
        text: "cd".to_string(),
        layout: LayoutTag::En,
        origin: RunOrigin::Physical,
        kind: RunKind::Text,
    });

    let runs = ring_buffer::runs_snapshot();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "abcd");
}

#[test]
fn run_journal_splits_on_layout_change() {
    ring_buffer::invalidate();
    ring_buffer::push_run(InputRun {
        text: "ABC".to_string(),
        layout: LayoutTag::En,
        origin: RunOrigin::Physical,
        kind: RunKind::Text,
    });
    ring_buffer::push_run(InputRun {
        text: ",".to_string(),
        layout: LayoutTag::Ru,
        origin: RunOrigin::Physical,
        kind: RunKind::Text,
    });

    let runs = ring_buffer::runs_snapshot();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].layout, LayoutTag::En);
    assert_eq!(runs[1].layout, LayoutTag::Ru);
}

#[test]
fn take_last_layout_run_with_suffix_basic() {
    ring_buffer::invalidate();
    ring_buffer::push_run(InputRun {
        text: "hello".to_string(),
        layout: LayoutTag::En,
        origin: RunOrigin::Physical,
        kind: RunKind::Text,
    });
    ring_buffer::push_run(InputRun {
        text: "   ".to_string(),
        layout: LayoutTag::En,
        origin: RunOrigin::Physical,
        kind: RunKind::Whitespace,
    });

    let (run, suffix) = ring_buffer::take_last_layout_run_with_suffix().expect("payload");
    assert_eq!(run.text, "hello");
    assert_eq!(suffix.len(), 1);
    assert_eq!(suffix[0].text, "   ");
}

#[test]
fn take_last_layout_run_with_suffix_returns_none_for_whitespace_only() {
    ring_buffer::invalidate();
    ring_buffer::push_text("  \t\n");
    assert!(ring_buffer::take_last_layout_run_with_suffix().is_none());
}

#[test]
fn legacy_push_text_segments_internally() {
    ring_buffer::invalidate();
    ring_buffer::push_text("hello world   ");

    let (run, suffix) = ring_buffer::take_last_layout_run_with_suffix().expect("payload");
    assert_eq!(run.text, "world");
    assert_eq!(
        suffix.iter().map(|r| r.text.as_str()).collect::<String>(),
        "   "
    );
}

#[test]
fn last_char_triggers_autoconvert_still_works_across_runs() {
    ring_buffer::invalidate();
    ring_buffer::push_run(InputRun {
        text: "abc".to_string(),
        layout: LayoutTag::En,
        origin: RunOrigin::Physical,
        kind: RunKind::Text,
    });
    ring_buffer::push_run(InputRun {
        text: ".".to_string(),
        layout: LayoutTag::En,
        origin: RunOrigin::Physical,
        kind: RunKind::Text,
    });
    assert!(ring_buffer::last_char_triggers_autoconvert());

    ring_buffer::push_run(InputRun {
        text: " ".to_string(),
        layout: LayoutTag::En,
        origin: RunOrigin::Physical,
        kind: RunKind::Whitespace,
    });
    assert!(ring_buffer::last_char_triggers_autoconvert());
}

#[test]
fn backspace_removes_from_last_run_and_drops_empty_run() {
    ring_buffer::invalidate();
    ring_buffer::push_text("a");
    ring_buffer::test_backspace();
    assert!(ring_buffer::take_last_layout_run_with_suffix().is_none());
}

#[test]
fn foreground_invalidation_state_reset_via_invalidate() {
    ring_buffer::invalidate();
    ring_buffer::push_text("abc");
    ring_buffer::mark_last_token_autoconverted();
    assert!(ring_buffer::last_token_autoconverted());

    ring_buffer::invalidate();
    assert!(!ring_buffer::last_token_autoconverted());
    assert!(ring_buffer::take_last_layout_run_with_suffix().is_none());
}
