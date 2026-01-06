#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionOutcome {
    Applied,
    Skipped(SkipReason),
    Failed(Failure),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SkipReason {
    NoSelection,
    ClipboardEmpty,
    NotText,
    SelectionTooLong,
    SelectionMultiline,
    NotImproved,
    RateLimited,
    Reentry,
    ForegroundUnavailable,
    ShiftPressed,
    NoLastWord,
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
    pub fn as_str(self) -> &'static str {
        match self {
            SkipReason::NoSelection => "no_selection",
            SkipReason::ClipboardEmpty => "clipboard_empty",
            SkipReason::NotText => "not_text",
            SkipReason::SelectionTooLong => "selection_too_long",
            SkipReason::SelectionMultiline => "selection_multiline",
            SkipReason::NotImproved => "not_improved",
            SkipReason::RateLimited => "rate_limited",
            SkipReason::Reentry => "reentry",
            SkipReason::ForegroundUnavailable => "foreground_unavailable",
            SkipReason::ShiftPressed => "shift_pressed",
            SkipReason::NoLastWord => "no_last_word",
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    ClipboardError,
    LayoutError,
    WinApiError,
    InputError,
}
