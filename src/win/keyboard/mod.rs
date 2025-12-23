mod capture;
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
        CallNextHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW, WH_KEYBOARD_LL,
    },
};

use self::vk::{is_keydown_msg, is_keyup_msg, mod_bit_for_vk, normalize_vk};
use crate::win::keyboard::{keydown::handle_keydown, keyup::handle_keyup};

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
    crate::ui::error_notifier::push(
        hwnd,
        state,
        crate::ui::error_notifier::T_UI,
        "Hotkey handling failed",
        e,
    );
}

extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        let h = HOOK_HANDLE.load(Ordering::Relaxed);
        let hook = (h != 0).then_some(HHOOK(h as *mut _));
        return unsafe { CallNextHookEx(hook, code, wparam, lparam) };
    }

    let msg = wparam.0 as u32;
    let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let vk = normalize_vk(kb);
    let is_mod = mod_bit_for_vk(vk).is_some();

    let decision = if is_keydown_msg(msg) {
        handle_keydown(vk, is_mod)
    } else if is_keyup_msg(msg) {
        handle_keyup(vk, is_mod)
    } else {
        Ok(HookDecision::Pass)
    };

    if is_keydown_msg(msg) && matches!(decision.as_ref(), Ok(HookDecision::Pass)) {
        crate::input_journal::record_keydown(kb, vk);
    }

    match decision {
        Ok(d) if d.should_swallow() && !(is_mod && is_keyup_msg(msg)) => return LRESULT(1),
        Ok(_) => {}
        Err(e) => {
            if let Some(hwnd) = main_hwnd() {
                super::with_state_mut_do(hwnd, |state| {
                    report_hook_error(hwnd, state, &e);
                });
            }
        }
    }

    let h = HOOK_HANDLE.load(Ordering::Relaxed);
    let hook = (h != 0).then_some(HHOOK(h as *mut _));
    unsafe { CallNextHookEx(hook, code, wparam, lparam) }
}

pub fn install(hwnd: HWND) {
    MAIN_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

    if HOOK_HANDLE.load(Ordering::Relaxed) != 0 {
        return;
    }

    match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), None, 0) } {
        Ok(h) => {
            HOOK_HANDLE.store(h.0 as isize, Ordering::Relaxed);
            #[cfg(debug_assertions)]
            eprintln!("RustSwitcher: WH_KEYBOARD_LL installed");
        }
        Err(_e) => {
            #[cfg(debug_assertions)]
            eprintln!("RustSwitcher: SetWindowsHookExW failed: {}", _e);
        }
    }
}
