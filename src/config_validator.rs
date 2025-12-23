use crate::config::Config;
use crate::constants::*;

pub fn find_duplicate_hotkey_sequences(config: &Config) -> Option<String> {
    let sequences = [
        (CONVERT_LAST_WORD, &config.hotkey_convert_last_word_sequence),
        (PAUSE, &config.hotkey_pause_sequence),
        (CONVERT_SELECTION, &config.hotkey_convert_selection_sequence),
        (SWITCH_LAYOUT, &config.hotkey_switch_layout_sequence),
    ];
    
    // Allowed duplicates (bidirectional check)
    let is_allowed_duplicate = |a: &str, b: &str| {
        (a == CONVERT_SELECTION && b == CONVERT_LAST_WORD) ||
        (a == CONVERT_LAST_WORD && b == CONVERT_SELECTION)
    };

    let duplicates: Vec<_> = sequences
        .iter()
        .enumerate()
        .flat_map(|(i, (name1, seq1_opt))| {
            seq1_opt.as_ref().map(|seq1| {
                sequences
                    .iter()
                    .enumerate()
                    .skip(i + 1)
                    .filter_map(move |(_j, (name2, seq2_opt))| {
                        seq2_opt
                            .as_ref()
                            .filter(|seq2| seq1 == *seq2 && !is_allowed_duplicate(name1, name2))
                            .map(|_| (*name1, *name2))
                    })
            })
        })
        .flatten()
        .collect();

    if duplicates.is_empty() {
        None
    } else {
        let mut error = String::from("Duplicate hotkey sequences found:\n\n");
        
        for (name1, name2) in &duplicates {
            error.push_str(&format!("• '{}' and '{}'\n", name1, name2));
        }
        
        error.push_str("\nEach action must have a unique hotkey sequence.");
        Some(error)
    }
}

impl Config {
    pub fn validate_hotkey_sequences(&self) -> Result<(), String> {
        if let Some(error) = find_duplicate_hotkey_sequences(self) {
            Err(error)
        } else {
            Ok(())
        }
    }
}
