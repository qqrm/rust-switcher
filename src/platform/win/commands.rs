use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::{BN_CLICKED, DestroyWindow, EN_KILLFOCUS, EN_SETFOCUS},
};

use super::state::with_state_mut_do;
use crate::{app::ControlId, platform::ui::error_notifier::T_UI, utils::helpers};

#[cfg_attr(
    debug_assertions,
    tracing::instrument(level = "info", skip_all, fields(msg, id, notif))
)]
pub(crate) fn on_command(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    #[cfg(debug_assertions)]
    tracing::Span::current().record("msg", "WM_COMMAND");

    let id = i32::from(helpers::loword(wparam.0));
    let notif = u32::from(helpers::hiword(wparam.0));

    #[cfg(debug_assertions)]
    {
        tracing::Span::current().record("id", id);
        tracing::Span::current().record("notif", i64::from(notif));
        eprintln!("ui.command: id={} notif={} lparam={}", id, notif, lparam.0);
    }

    if let Some(r) = handle_hotkey_capture_focus(hwnd, id, notif) {
        return r;
    }

    if notif != BN_CLICKED {
        return LRESULT(0);
    }

    handle_buttons(hwnd, id)
}

fn handle_hotkey_capture_focus(hwnd: HWND, id: i32, notif: u32) -> Option<LRESULT> {
    let cid = ControlId::from_i32(id)?;

    let slot = match cid {
        ControlId::HotkeyLastWord => crate::app::HotkeySlot::LastWord,
        ControlId::HotkeyPause => crate::app::HotkeySlot::Pause,
        ControlId::HotkeySelection => crate::app::HotkeySlot::Selection,
        ControlId::HotkeySwitchLayout => crate::app::HotkeySlot::SwitchLayout,
        _ => return None,
    };

    match notif {
        EN_SETFOCUS => {
            with_state_mut_do(hwnd, |state| {
                state.hotkey_capture.active = true;
                state.hotkey_capture.slot = Some(slot);
                state.hotkey_capture.pending_mods_vks = 0;
                state.hotkey_capture.pending_mods = 0;
                state.hotkey_capture.pending_mods_valid = false;
                state.hotkey_capture.saw_non_mod = false;
                state.hotkey_capture.last_input_tick_ms = 0;

                #[cfg(debug_assertions)]
                eprintln!("hotkey.capture: start slot={slot:?}");
            });
            Some(LRESULT(0))
        }

        EN_KILLFOCUS => {
            with_state_mut_do(hwnd, |state| {
                state.hotkey_capture.active = false;

                #[cfg(debug_assertions)]
                eprintln!("hotkey.capture: stop slot={slot:?}");
            });
            Some(LRESULT(0))
        }

        _ => Some(LRESULT(0)),
    }
}

fn handle_buttons(hwnd: HWND, id: i32) -> LRESULT {
    let Some(cid) = ControlId::from_i32(id) else {
        return LRESULT(0);
    };

    match cid {
        ControlId::Exit => with_state_mut_do(hwnd, |state| {
            if let Err(e) = unsafe { DestroyWindow(hwnd) } {
                crate::platform::ui::error_notifier::push(
                    hwnd,
                    state,
                    T_UI,
                    "Failed to close the window",
                    &e,
                );
                super::on_app_error(hwnd);
            }
        }),

        ControlId::Apply => with_state_mut_do(hwnd, |state| super::handle_apply(hwnd, state)),
        ControlId::Cancel => with_state_mut_do(hwnd, |state| super::handle_cancel(hwnd, state)),

        _ => {}
    }

    LRESULT(0)
}
