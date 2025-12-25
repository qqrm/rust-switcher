use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GWLP_USERDATA, GetWindowLongPtrW},
};

use crate::app::AppState;

#[cfg(test)]
pub(crate) fn with_state_mut<R>(_hwnd: HWND, f: impl FnOnce(&mut AppState) -> R) -> Option<R> {
    // Create a dummy state just for the test
    let mut dummy_state = AppState::default();
    Some(f(&mut dummy_state))
}

#[cfg(not(test))]
pub(crate) fn with_state_mut<R>(hwnd: HWND, f: impl FnOnce(&mut AppState) -> R) -> Option<R> {
    unsafe {
        let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
        (!p.is_null()).then(|| f(&mut *p))
    }
}

pub(crate) fn with_state_mut_do(hwnd: HWND, f: impl FnOnce(&mut AppState)) {
    unsafe {
        let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
        if !p.is_null() {
            f(&mut *p);
        }
    }
}
