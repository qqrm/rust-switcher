#[cfg(windows)]
pub mod convert;
#[cfg(windows)]
pub mod last_word;
pub mod mapping;

#[cfg(windows)]
pub use convert::{switch_keyboard_layout, wait_shift_released};
#[cfg(windows)]
pub use last_word::autoconvert_last_word;
pub use mapping::{ConversionDirection, convert_ru_en_with_direction};
