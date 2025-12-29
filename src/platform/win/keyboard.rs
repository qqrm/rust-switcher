mod capture;
#[cfg(debug_assertions)]
pub(crate) mod debug_timers;
mod keydown;
mod keyup;
pub(crate) mod mods;
mod sequence;
mod vk;

use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::SystemInformation::GetTickCount64,
    UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, PostMessageW, SetWindowsHookExW,
        WH_KEYBOARD_LL,
    },
};

use self::vk::{is_keydown_msg, is_keyup_msg, mod_bit_for_vk, normalize_vk};
use crate::{
    input,
    platform::win::keyboard::{keydown::handle_keydown, keyup::handle_keyup},
};

static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

fn now_tick_ms() -> u64 {
    unsafe { GetTickCount64() }
}

fn main_hwnd() -> Option<HWND> {
    let raw = MAIN_HWND.load(Ordering::Relaxed);
    if raw == 0 {
        None
    } else {
        Some(HWND(raw as *mut _))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum HookDecision {
    Pass,
    Swallow,
}

impl HookDecision {
    fn should_swallow(self) -> bool {
        matches!(self, Self::Swallow)
    }
}

fn report_hook_error(hwnd: HWND, state: &mut crate::app::AppState, e: &windows::core::Error) {
    crate::platform::ui::error_notifier::push(
        hwnd,
        state,
        crate::platform::ui::error_notifier::T_UI,
        "Hotkey handling failed",
        e,
    );
}

extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION.cast_signed() {
        let h = HOOK_HANDLE.load(Ordering::Relaxed);
        let hook = (h != 0).then_some(HHOOK(h as *mut _));
        return unsafe { CallNextHookEx(hook, code, wparam, lparam) };
    }

    let h = HOOK_HANDLE.load(Ordering::Relaxed);
    let hook = (h != 0).then_some(HHOOK(h as *mut _));

    let Ok(msg) = u32::try_from(wparam.0) else {
        return unsafe { CallNextHookEx(hook, code, wparam, lparam) };
    };

    let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let vk = normalize_vk(kb);
    let is_mod = mod_bit_for_vk(vk).is_some();

    let is_keydown = is_keydown_msg(msg);
    let is_keyup = is_keyup_msg(msg);

    let decision = if is_keydown {
        handle_keydown(vk, is_mod)
    } else if is_keyup {
        handle_keyup(vk, is_mod)
    } else {
        Ok(HookDecision::Pass)
    };

    if is_keydown && matches!(decision.as_ref(), Ok(HookDecision::Pass)) {
        let typed = input::ring_buffer::record_keydown(kb, vk);

        if typed.is_some()
            && crate::input::ring_buffer::last_char_triggers_autoconvert()
            && let Some(hwnd) = main_hwnd()
        {
            let _ = unsafe {
                PostMessageW(
                    Some(hwnd),
                    crate::platform::ui::error_notifier::WM_APP_AUTOCONVERT,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
    }

    match decision {
        Ok(d) if d.should_swallow() && !(is_mod && is_keyup) => return LRESULT(1),
        Ok(_) => {}
        Err(e) => {
            if let Some(hwnd) = main_hwnd() {
                super::with_state_mut_do(hwnd, |state| {
                    report_hook_error(hwnd, state, &e);
                });
            }
        }
    }

    unsafe { CallNextHookEx(hook, code, wparam, lparam) }
}

/// Installs the low level keyboard hook used for hotkey capture.
///
/// On failure, the error is routed through the UI error notifier. This keeps the
/// release build observable even without logs.
pub fn install(hwnd: HWND, state: &mut crate::app::AppState) {
    MAIN_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

    if HOOK_HANDLE.load(Ordering::Relaxed) != 0 {
        return;
    }

    match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), None, 0) } {
        Ok(h) => {
            HOOK_HANDLE.store(h.0 as isize, Ordering::Relaxed);
            #[cfg(debug_assertions)]
            tracing::info!("WH_KEYBOARD_LL installed");
        }
        Err(e) => {
            crate::platform::ui::error_notifier::push(
                hwnd,
                state,
                crate::platform::ui::error_notifier::T_UI,
                "Failed to install keyboard hook",
                &e,
            );
        }
    }
}
