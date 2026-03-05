use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GWLP_USERDATA, GetWindowLongPtrW},
};

use crate::app::AppState;

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

pub(crate) fn get_state(hwnd: HWND) -> Option<&'static AppState> {
    unsafe {
        let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
        if !p.is_null() { Some(&*p) } else { None }
    }
}
