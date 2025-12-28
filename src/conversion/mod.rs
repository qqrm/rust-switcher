pub mod clipboard;
pub mod input;

pub use crate::domain::text::{
    convert::{convert_selection, convert_selection_if_any},
    last_word::convert_last_word,
};
