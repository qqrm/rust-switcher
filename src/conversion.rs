use crate::app::AppState;
use windows::Win32::Foundation::HWND;

pub fn convert_last_word(state: &mut AppState, _hwnd: HWND) {
    let delay = unsafe { crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100) };
    println!("convert_last_word delay={}", delay);
}

pub fn convert_selection(state: &mut AppState, _hwnd: HWND) {
    let delay = unsafe { crate::helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(100) };
    println!("convert_selection delay={}", delay);
}

pub fn switch_keyboard_layout() {
    println!("switch_keyboard_layout");
}
