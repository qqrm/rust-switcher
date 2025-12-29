use windows::Win32::{
    Foundation::{HWND, LRESULT},
    UI::WindowsAndMessaging::{KillTimer, SetTimer},
};

use crate::{
    platform::win::state::with_state_mut,
    utils::helpers::{DEBUG_TIMER_ID_STARTUP_ERROR, DEBUG_TIMER_ID_STARTUP_INFO, last_error},
};

pub fn handle_timer(hwnd: HWND, id: usize) -> Option<LRESULT> {
    if id == DEBUG_TIMER_ID_STARTUP_ERROR {
        unsafe {
            let _ = KillTimer(
                Some(hwnd),
                crate::utils::helpers::DEBUG_TIMER_ID_STARTUP_ERROR,
            );
        }

        with_state_mut(hwnd, |state| {
            let e = last_error();
            crate::platform::ui::error_notifier::push(
                hwnd,
                state,
                "Test title в›‘пёЏ",
                "Startup test error",
                &e,
            );
        });

        unsafe {
            let _ = SetTimer(Some(hwnd), DEBUG_TIMER_ID_STARTUP_INFO, 600, None);
        }

        return Some(LRESULT(0));
    }

    if id == crate::utils::helpers::DEBUG_TIMER_ID_STARTUP_INFO {
        unsafe {
            let _ = KillTimer(Some(hwnd), DEBUG_TIMER_ID_STARTUP_INFO);
        }

        with_state_mut(hwnd, |state| {
            crate::platform::ui::info_notifier::push(
                hwnd,
                state,
                "Test title",
                "Notification test info",
            );
        });

        return Some(LRESULT(0));
    }

    None
}
