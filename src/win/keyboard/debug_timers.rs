use windows::Win32::{
    Foundation::{HWND, LRESULT},
    UI::WindowsAndMessaging::{KillTimer, SetTimer},
};

use crate::win::state::with_state_mut;

pub fn handle_timer(hwnd: HWND, id: usize) -> Option<LRESULT> {
    if id == crate::helpers::DEBUG_TIMER_ID_STARTUP_ERROR {
        unsafe {
            let _ = KillTimer(Some(hwnd), crate::helpers::DEBUG_TIMER_ID_STARTUP_ERROR);
        }

        with_state_mut(hwnd, |state| {
            let e = crate::helpers::last_error();
            crate::ui::error_notifier::push(hwnd, state, "Test title ⛑️", "Startup test error", &e);
        });

        unsafe {
            let _ = SetTimer(
                Some(hwnd),
                crate::helpers::DEBUG_TIMER_ID_STARTUP_INFO,
                600,
                None,
            );
        }

        return Some(LRESULT(0));
    }

    if id == crate::helpers::DEBUG_TIMER_ID_STARTUP_INFO {
        unsafe {
            let _ = KillTimer(Some(hwnd), crate::helpers::DEBUG_TIMER_ID_STARTUP_INFO);
        }

        with_state_mut(hwnd, |state| {
            crate::ui::info_notifier::push(hwnd, state, "Test title ⛑️", "Notification test info");
        });

        return Some(LRESULT(0));
    }

    None
}
