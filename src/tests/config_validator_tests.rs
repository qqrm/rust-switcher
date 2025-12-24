
    #[cfg(test)]
    mod tests {
        use crate::config::HotkeySequence;
        use crate::config::*;
        use crate::config_validator::find_duplicate_hotkey_sequences;
        use crate::constants::*;

        // Helper function to create a test config
        fn create_test_config(
            last_word: Option<HotkeySequence>,
            pause: Option<HotkeySequence>,
            selection: Option<HotkeySequence>,
            layout: Option<HotkeySequence>,
        ) -> Config {
            Config {
                hotkey_convert_last_word_sequence: last_word,
                hotkey_pause_sequence: pause,
                hotkey_convert_selection_sequence: selection,
                hotkey_switch_layout_sequence: layout,
                ..Default::default() // assuming Config has other fields with Default impl
            }
        }

        fn seq(value: &str) -> Option<HotkeySequence> {
        // Parse keys properly with LCtrl/RCtrl support
        let normalized = value.to_uppercase();
        
        let vk_code = if normalized.contains("SPACE") {
            32 // VK_SPACE
        } else {
            // Extract the last key after the last '+'
            let last_key = normalized.split('+').last().unwrap_or("");
            match last_key {
                "A" => 65,
                "B" => 66,
                "C" => 67,
                "D" => 68,
                "X" => 88,
                "Y" => 89,
                "Z" => 90,
                "L" => 76, // For "L" key (not LCtrl modifier)
                "P" => 80,
                _ => 65, // default
            }
        };
        
        // Parse modifiers
        let mut mods = 0;
        if normalized.contains("LCTRL") || normalized.contains("RCTRL") || normalized.contains("CTRL") {
            mods |= 0x0002; // MOD_CONTROL
        }
        if normalized.contains("LSHIFT") || normalized.contains("RSHIFT") || normalized.contains("SHIFT") {
            mods |= 0x0001; // MOD_SHIFT
        }
        if normalized.contains("LALT") || normalized.contains("RALT") || normalized.contains("ALT") {
            mods |= 0x0004; // MOD_ALT
        }
        if normalized.contains("LWIN") || normalized.contains("RWIN") || normalized.contains("WIN") {
            mods |= 0x0008; // MOD_WIN
        }
        
        Some(HotkeySequence {
            first: HotkeyChord {
                mods: mods as u32,
                mods_vks: 0,
                vk: Some(vk_code),
            },
            second: None,
            max_gap_ms: 500,
        })
    }

    #[test]
    fn test_no_duplicates() {
        let config = create_test_config(
            seq("Ctrl+A"),
            seq("Ctrl+B"),
            seq("Ctrl+C"),
            seq("Ctrl+D"),
        );
        
        assert!(find_duplicate_hotkey_sequences(&config).is_none());
    }

    #[test]
    fn test_no_hotkeys_set() {
        let config = create_test_config(None, None, None, None);
        
        assert!(find_duplicate_hotkey_sequences(&config).is_none());
    }

    #[test]
    fn test_some_hotkeys_not_set() {
        let config = create_test_config(
            seq("Ctrl+A"),
            None,
            seq("Ctrl+B"),
            None,
        );
        
        assert!(find_duplicate_hotkey_sequences(&config).is_none());
    }

    #[test]
    fn test_duplicate_pause_and_layout() {
        let config = create_test_config(
            seq("Ctrl+A"),
            seq("Ctrl+X"),
            seq("Ctrl+B"),
            seq("Ctrl+X"), // Same as pause
        );
        
        let result = find_duplicate_hotkey_sequences(&config);
        assert!(result.is_some());
        
        let error = result.unwrap();
        assert!(error.contains(PAUSE));
        assert!(error.contains(SWITCH_LAYOUT));
        assert!(!error.contains(CONVERT_LAST_WORD));
        assert!(!error.contains(CONVERT_SELECTION));
    }

    #[test]
    fn test_duplicate_last_word_and_pause() {
        let config = create_test_config(
            seq("Ctrl+Shift+L"),
            seq("Ctrl+Shift+L"), // Same as last_word
            seq("Ctrl+C"),
            seq("Ctrl+D"),
        );
        
        let result = find_duplicate_hotkey_sequences(&config);
        assert!(result.is_some());
        
        let error = result.unwrap();
        assert!(error.contains(CONVERT_LAST_WORD));
        assert!(error.contains(PAUSE));
    }

    #[test]
    fn test_multiple_duplicates() {
        let config = create_test_config(
            seq("Ctrl+X"), // Duplicate 1
            seq("Ctrl+X"), // Duplicate 1
            seq("Ctrl+Y"), // Duplicate 2
            seq("Ctrl+Y"), // Duplicate 2
        );
        
        let result = find_duplicate_hotkey_sequences(&config);
        assert!(result.is_some());
        
        let error = result.unwrap();
        // Should report both duplicate pairs
        assert!(error.matches(CONVERT_LAST_WORD).count() >= 1);
        assert!(error.matches(PAUSE).count() >= 1);
        assert!(error.matches(CONVERT_SELECTION).count() >= 1);
        assert!(error.matches(SWITCH_LAYOUT).count() >= 1);
    }

    #[test]
    fn test_allowed_duplicate_selection_and_last_word() {
        // These two are allowed to be duplicates
        let config = create_test_config(
            seq("Ctrl+Space"), // Last word
            seq("Ctrl+P"),     // Pause (unique)
            seq("Ctrl+Space"), // Selection (same as last word - allowed!)
            seq("Ctrl+L"),     // Layout (unique)
        );
        
        // Should NOT return an error since these are allowed duplicates
        assert!(find_duplicate_hotkey_sequences(&config).is_none());
    }

    #[test]
    fn test_allowed_duplicate_but_other_duplicate_exists() {
        // Last word and selection are allowed to be same,
        // but pause and layout should not be same
        let config = create_test_config(
            seq("Ctrl+Space"), // Last word (allowed duplicate)
            seq("Ctrl+X"),     // Pause
            seq("Ctrl+Space"), // Selection (allowed duplicate)
            seq("Ctrl+X"),     // Layout (duplicate with pause - NOT allowed!)
        );
        
        let result = find_duplicate_hotkey_sequences(&config);
        assert!(result.is_some());
        
        let error = result.unwrap();
        // Should report pause and layout duplicate
        assert!(error.contains(PAUSE));
        assert!(error.contains(SWITCH_LAYOUT));
        // Should NOT mention the allowed duplicates
        assert!(!error.contains(CONVERT_LAST_WORD) || !error.contains(CONVERT_SELECTION));
    }

    #[test]
    fn test_different_letters_no_duplicate() {
        // All different letters after normalization
        let config = create_test_config(
            seq("ctrl+a"),  // → CTRL+A
            seq("Ctrl+B"),  // → CTRL+B
            seq("CTRL+C"),  // → CTRL+C
            seq("ctrl+D"),  // → CTRL+D
        );
        // No duplicates - all different letters
        assert!(find_duplicate_hotkey_sequences(&config).is_none());
    }

    #[test]
    fn test_whitespace_handling() {
        // Test if whitespace matters in comparison
        let config = create_test_config(
            seq("Ctrl + A"), // with spaces
            seq("Ctrl+A"),   // without spaces
            seq("Ctrl+B"),
            seq("Ctrl+C"),
        );
        
        // Result depends on how HotkeySequence handles equality
        // This test helps document the behavior
        let result = find_duplicate_hotkey_sequences(&config);
        // Either result is fine, but we should know which it is
        println!("Result: {:?}", result);
    }

    #[test]
    fn test_error_message_format() {
        let config = create_test_config(
            seq("Ctrl+X"),
            seq("Ctrl+X"),
            seq("Ctrl+Y"),
            seq("Ctrl+Y"),
        );
        
        let error = find_duplicate_hotkey_sequences(&config).unwrap();
        
        // Check error message format
        assert!(error.starts_with("Duplicate hotkey sequences found:"));
        assert!(error.contains("•"));
        assert!(error.contains("and"));
        assert!(error.ends_with("Each action must have a unique hotkey sequence."));
        
        // Should have newlines for readability
        assert!(error.contains('\n'));
    }

    #[test]
    fn test_single_duplicate_pair() {
        let config = create_test_config(
            seq("Ctrl+A"),
            seq("Ctrl+B"),
            seq("Ctrl+C"),
            seq("Ctrl+A"), // Duplicate with first one
        );
        
        let result = find_duplicate_hotkey_sequences(&config);
        assert!(result.is_some());
        
        let error = result.unwrap();
        // Count the number of duplicate pairs mentioned
        let pair_count = error.matches("•").count();
        assert_eq!(pair_count, 1, "Should report exactly 1 duplicate pair");
    }

    #[test]
    fn test_empty_sequences() {
        // Test with empty string sequences if that's allowed
        let config = create_test_config(
            seq(""),
            seq(""),
            seq("Ctrl+A"),
            seq("Ctrl+B"),
        );
        
        let result = find_duplicate_hotkey_sequences(&config);
        // Result depends on whether empty sequences are considered equal
        println!("Empty sequence test result: {:?}", result);
    }
}