//! Window creation and message dispatching.
//!
//! The `win` module encapsulates the main window procedure and the
//! event loop for the application.  It wires together the helper
//! routines, the application state, and the UI construction code to
//! present a settings window and respond to user actions.

mod autostart;
mod commands;
pub(crate) mod hotkey_format;
pub(crate) mod keyboard;
pub(crate) mod mouse;
mod state;
pub(crate) mod tray;
mod tray_dispatch;
mod visuals;
mod window;
pub(crate) use hotkey_format::{format_hotkey, format_hotkey_sequence};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{DeleteObject, HFONT, HGDIOBJ},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, IsWindowVisible, PostQuitMessage,
            SC_CLOSE, SW_HIDE, SW_SHOW, SetWindowLongPtrW, ShowWindow, WM_CLOSE, WM_COMMAND,
            WM_CREATE, WM_CTLCOLORBTN, WM_CTLCOLORDLG, WM_CTLCOLORSTATIC, WM_DESTROY, WM_HOTKEY,
            WM_SYSCOMMAND, WM_TIMER, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
        },
    },
    core::{PCWSTR, Result, w},
};

use self::{
    state::{with_state_mut, with_state_mut_do},
    window::{
        compute_window_size, create_main_window, message_loop, register_main_class,
        set_window_icons,
    },
};
pub(crate) const AUTOSTART_ARG: &str = "--autostart";
use crate::{
    app::AppState,
    config,
    domain::text::{last_word::autoconvert_last_word, switch_keyboard_layout},
    input::hotkeys::{HotkeyAction, action_from_id},
    platform::{
        ui::{
            self,
            colors::on_ctlcolor,
            error_notifier::{T_CONFIG, T_UI, drain_one_and_present},
            notify::on_wm_app_notify,
        },
        win::{
            tray::{WM_APP_TRAY, remove_icon},
            tray_dispatch::handle_tray_timer,
        },
    },
    ui_call, ui_try,
    utils::helpers,
};

#[rustfmt::skip]
#[cfg(debug_assertions)]
use crate::platform::win::keyboard::debug_timers::handle_timer;

fn set_hwnd_text(hwnd: HWND, s: &str) -> windows::core::Result<()> {
    helpers::set_edit_text(hwnd, s)
}

pub fn refresh_autostart_checkbox(state: &mut AppState) -> windows::core::Result<()> {
    let enabled = crate::platform::win::autostart::is_enabled()?;
    crate::utils::helpers::set_checkbox(state.checkboxes.autostart, enabled);
    Ok(())
}

fn apply_config_to_ui(state: &mut AppState, cfg: &config::Config) -> windows::core::Result<()> {
    helpers::set_edit_u32(state.edits.delay_ms, cfg.delay_ms)?;

    state.hotkey_values = crate::app::HotkeyValues::from_config(cfg);
    state.hotkey_sequence_values = crate::app::HotkeySequenceValues::from_config(cfg);

    let last_word_text = if cfg.hotkey_convert_last_word_sequence.is_some() {
        format_hotkey_sequence(cfg.hotkey_convert_last_word_sequence)
    } else {
        format_hotkey(cfg.hotkey_convert_last_word)
    };
    set_hwnd_text(state.hotkeys.last_word, &last_word_text)?;

    let pause_text = if cfg.hotkey_pause_sequence.is_some() {
        format_hotkey_sequence(cfg.hotkey_pause_sequence)
    } else {
        format_hotkey(cfg.hotkey_pause)
    };
    set_hwnd_text(state.hotkeys.pause, &pause_text)?;

    let selection_text = if cfg.hotkey_convert_selection_sequence.is_some() {
        format_hotkey_sequence(cfg.hotkey_convert_selection_sequence)
    } else {
        format_hotkey(cfg.hotkey_convert_selection)
    };
    set_hwnd_text(state.hotkeys.selection, &selection_text)?;

    let switch_layout_text = if cfg.hotkey_switch_layout_sequence.is_some() {
        format_hotkey_sequence(cfg.hotkey_switch_layout_sequence)
    } else {
        format_hotkey(cfg.hotkey_switch_layout)
    };
    set_hwnd_text(state.hotkeys.switch_layout, &switch_layout_text)?;

    Ok(())
}

fn read_ui_to_config(state: &AppState, mut cfg: config::Config) -> config::Config {
    cfg.delay_ms = helpers::get_edit_u32(state.edits.delay_ms).unwrap_or(cfg.delay_ms);

    cfg.hotkey_convert_last_word_sequence = state.hotkey_sequence_values.last_word;
    cfg.hotkey_pause_sequence = state.hotkey_sequence_values.pause;
    cfg.hotkey_convert_selection_sequence = state.hotkey_sequence_values.selection;
    cfg.hotkey_switch_layout_sequence = state.hotkey_sequence_values.switch_layout;

    fn hk_or_none_if_double(
        seq: Option<config::HotkeySequence>,
        hk: Option<config::Hotkey>,
    ) -> Option<config::Hotkey> {
        match seq {
            Some(s) if s.second.is_some() => None,
            _ => hk,
        }
    }

    cfg.hotkey_convert_last_word = hk_or_none_if_double(
        cfg.hotkey_convert_last_word_sequence,
        state.hotkey_values.last_word,
    );
    cfg.hotkey_pause = hk_or_none_if_double(cfg.hotkey_pause_sequence, state.hotkey_values.pause);
    cfg.hotkey_convert_selection = hk_or_none_if_double(
        cfg.hotkey_convert_selection_sequence,
        state.hotkey_values.selection,
    );
    cfg.hotkey_switch_layout = match cfg.hotkey_switch_layout_sequence {
        Some(_) => None,
        None => state.hotkey_values.switch_layout,
    };

    cfg
}

fn apply_config_runtime(
    hwnd: HWND,
    state: &mut AppState,
    cfg: &config::Config,
) -> windows::core::Result<()> {
    state.autoconvert_enabled = false;

    state.active_hotkey_sequences = crate::app::HotkeySequenceValues::from_config(cfg);

    state.runtime_chord_capture = crate::app::RuntimeChordCapture::default();
    state.hotkey_sequence_progress = crate::app::HotkeySequenceProgress::default();

    state.active_switch_layout_sequence = cfg.hotkey_switch_layout_sequence;
    state.switch_layout_waiting_second = false;
    state.switch_layout_first_tick_ms = 0;

    ui_try!(
        hwnd,
        state,
        T_CONFIG,
        "Failed to register hotkeys",
        crate::input::hotkeys::register_from_config(hwnd, cfg)
    );

    Ok(())
}

fn init_font_and_visuals(hwnd: HWND, state: &mut AppState) {
    unsafe {
        match visuals::create_message_font() {
            Ok(font) => state.font = font,
            Err(_) => state.font = HFONT::default(),
        }
        if !state.font.0.is_null() {
            visuals::apply_modern_look(hwnd, state.font);
        }
    }
}

macro_rules! startup_or_return0 {
    ($hwnd:expr, $state:expr, $text:expr, $expr:expr) => {{
        match $expr {
            Ok(v) => v,
            Err(e) => {
                $crate::platform::ui::error_notifier::push($hwnd, $state, "", $text, &e);
                on_app_error($hwnd);
                return LRESULT(0);
            }
        }
    }};
}

pub fn handle_autostart_toggle(hwnd: HWND, state: &mut AppState) {
    let desired = crate::utils::helpers::get_checkbox(state.checkboxes.autostart);

    if let Err(e) = crate::platform::win::autostart::apply_startup_shortcut(desired) {
        crate::platform::ui::error_notifier::push(
            hwnd,
            state,
            T_UI,
            "Failed to update autostart setting",
            &e,
        );

        let _ = refresh_autostart_checkbox(state);
    }
}

/// Loads config and validates it for runtime use.
///
/// On invalid config, this function notifies the user and falls back to defaults.
/// This keeps the application operational even when the config file was edited manually.
fn load_config_or_default(hwnd: HWND, state: &mut AppState) -> config::Config {
    config::load()
        .map_err(|e| {
            crate::platform::ui::error_notifier::push(
                hwnd,
                state,
                T_CONFIG,
                "Failed to load config, using defaults",
                &io_to_win(e),
            );
        })
        .ok()
        .and_then(|cfg| match cfg.validate_hotkey_sequences() {
            Ok(()) => Some(cfg),
            Err(msg) => {
                let user_text = msg.clone();
                let source = io_to_win(std::io::Error::new(std::io::ErrorKind::InvalidInput, msg));
                crate::platform::ui::error_notifier::push(
                    hwnd, state, T_CONFIG, &user_text, &source,
                );
                None
            }
        })
        .unwrap_or_default()
}

fn on_create(hwnd: HWND) -> LRESULT {
    let mut state = Box::new(AppState::default());

    #[rustfmt::skip]
    startup_or_return0!(hwnd, &mut state, "Failed to create UI controls", ui::create_controls(hwnd, &mut state));
    let cfg = load_config_or_default(hwnd, state.as_mut());

    state.hotkey_values = crate::app::HotkeyValues::from_config(&cfg);
    state.active_hotkey_sequences = crate::app::HotkeySequenceValues::from_config(&cfg);

    #[rustfmt::skip] {
        startup_or_return0!(hwnd, &mut state, "Failed to apply config to UI", apply_config_to_ui(state.as_mut(), &cfg));
        startup_or_return0!(hwnd, &mut state, "Failed to read autostart state", refresh_autostart_checkbox(state.as_mut()));
        startup_or_return0!(hwnd, &mut state, "Failed to apply config at runtime", apply_config_runtime(hwnd, state.as_mut(), &cfg));
    }

    keyboard::install(hwnd, state.as_mut());
    mouse::install();

    init_font_and_visuals(hwnd, &mut state);

    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    }

    if let Err(e) = crate::platform::win::tray::ensure_icon(hwnd) {
        tracing::warn!(error = ?e, "tray ensure_icon failed");
    }

    #[cfg(debug_assertions)]
    with_state_mut_do(hwnd, |state| {
        helpers::debug_startup_notification(hwnd, state);
    });

    LRESULT(0)
}

/// Start the main window and enter the message loop.
///
/// This function is called from `main` after the single instance
/// guard has been acquired.  It performs all initialization that
/// requires unsafe code and returns any error to the caller.
pub fn run(start_hidden: bool) -> Result<()> {
    unsafe {
        visuals::init_visuals();
        let class_name = w!("RustSwitcherMainWindow");
        let hinstance = GetModuleHandleW(PCWSTR::null())?.into();

        register_main_class(class_name, hinstance)?;

        let style = WS_OVERLAPPEDWINDOW & !WS_THICKFRAME & !WS_MAXIMIZEBOX;
        let (window_w, window_h) = compute_window_size(style)?;

        let (x, y) = helpers::default_window_pos(window_w, window_h);

        let hwnd = create_main_window(class_name, hinstance, style, x, y, window_w, window_h)?;
        set_window_icons(hwnd, hinstance);
        let show_cmd = if start_hidden { SW_HIDE } else { SW_SHOW };
        let _ = ShowWindow(hwnd, show_cmd);

        message_loop()?;
    }
    Ok(())
}

/// The window procedure.  Handles creation, command and destroy
/// messages.  Any unhandled messages are forwarded to the default
/// procedure.
pub extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    const WM_NCDESTROY: u32 = 0x0082;

    match msg {
        WM_CREATE => on_create(hwnd),
        WM_COMMAND => commands::on_command(hwnd, wparam),
        WM_HOTKEY => on_hotkey(hwnd, wparam),
        WM_TIMER => on_timer(hwnd, wparam, lparam),

        WM_CTLCOLORDLG | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => on_ctlcolor(wparam, lparam),
        WM_SYSCOMMAND => {
            let cmd = wparam.0 & 0xFFF0usize;
            if cmd == SC_CLOSE as usize {
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CLOSE => {
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_NCDESTROY => unsafe { on_ncdestroy(hwnd) },

        crate::platform::ui::notify::WM_APP_NOTIFY => {
            on_wm_app_notify(hwnd);
            LRESULT(0)
        }

        crate::platform::ui::error_notifier::WM_APP_ERROR => {
            with_state_mut_do(hwnd, |state| {
                drain_one_and_present(hwnd, state);
            });
            LRESULT(0)
        }

        crate::platform::ui::error_notifier::WM_APP_AUTOCONVERT => {
            if crate::input::ring_buffer::last_token_autoconverted() {
                return LRESULT(0);
            }

            with_state_mut_do(hwnd, |state| {
                if state.autoconvert_enabled {
                    autoconvert_last_word(state);
                }
            });

            LRESULT(0)
        }

        WM_APP_TRAY => tray_dispatch::handle_tray_message(hwnd, wparam, lparam),

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn io_to_win(e: std::io::Error) -> windows::core::Error {
    use windows::core::{Error, HRESULT};
    Error::new(HRESULT(0x8000_4005_u32 as i32), e.to_string())
}

/// Error returned by `build_and_save_config_from_ui`.
///
/// This type carries two payloads:
/// - `user_text`: a short message intended to be shown to the user
/// - `source`: the underlying Win32 error used for debug details
struct ApplyConfigError {
    user_text: String,
    source: windows::core::Error,
}

impl ApplyConfigError {
    /// Enqueues the error into the UI notifier queue and schedules UI processing.
    ///
    /// This method is intentionally side effecting and should be called only from
    /// the UI boundary code (for example a window command handler).
    fn notify(self, hwnd: HWND, state: &mut AppState) {
        crate::platform::ui::error_notifier::push(
            hwnd,
            state,
            T_CONFIG,
            &self.user_text,
            &self.source,
        );
    }
}

fn build_and_save_config_from_ui(
    state: &mut AppState,
) -> std::result::Result<config::Config, ApplyConfigError> {
    // Apply must not depend on reading existing config file.
    // Build a fresh config from UI and overwrite on disk.
    let cfg = read_ui_to_config(state, config::Config::default());

    config::save(&cfg).map_err(|e| {
        let user_text = match e.kind() {
            std::io::ErrorKind::InvalidInput => e.to_string(),
            _ => "Failed to save config".to_string(),
        };

        ApplyConfigError {
            user_text,
            source: io_to_win(e),
        }
    })?;

    Ok(cfg)
}

/// Applies configuration changes requested by the UI.
///
/// Architectural boundary:
/// This handler is a UI boundary. It coordinates:
/// - persistence (load, merge, save)
/// - runtime application of the new config
/// - updating the UI to reflect the saved config
///
/// All user notifications are emitted here. Lower level helpers return `Result` only.
fn handle_apply(hwnd: HWND, state: &mut AppState) {
    let cfg = match build_and_save_config_from_ui(state) {
        Ok(cfg) => cfg,
        Err(e) => {
            e.notify(hwnd, state);
            return;
        }
    };

    ui_call!(
        hwnd,
        state,
        T_CONFIG,
        "Failed to apply config at runtime",
        apply_config_runtime(hwnd, state, &cfg)
    );

    ui_call!(
        hwnd,
        state,
        T_UI,
        "Failed to update UI from config",
        apply_config_to_ui(state, &cfg)
    );
}

fn handle_cancel(hwnd: HWND, state: &mut AppState) {
    let cfg = config::load().unwrap_or_default();

    ui_call!(
        hwnd,
        state,
        T_CONFIG,
        "Failed to apply config at runtime",
        apply_config_runtime(hwnd, state, &cfg)
    );

    ui_call!(
        hwnd,
        state,
        T_UI,
        "Failed to update UI from config",
        apply_config_to_ui(state, &cfg)
    );
}

unsafe fn on_ncdestroy(hwnd: HWND) -> LRESULT {
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
    if p.is_null() {
        return LRESULT(0);
    }

    remove_icon(hwnd);

    let state = unsafe { &mut *p };

    if !state.font.0.is_null() {
        let _ = unsafe { DeleteObject(HGDIOBJ(state.font.0)) };
    }

    drop(unsafe { Box::from_raw(p) });
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
    LRESULT(0)
}

pub fn hotkey_id_from_wparam(wparam: WPARAM) -> i32 {
    wparam.0 as i32
}

fn handle_pause_toggle(hwnd: HWND, state: &mut AppState) {
    let enabled = !state.autoconvert_enabled;
    set_autoconvert_enabled_from_tray(hwnd, state, enabled, true);
}

fn handle_convert_smart(state: &mut AppState) {
    if crate::conversion::convert_selection_if_any(state) {
        return;
    }
    crate::conversion::convert_last_word(state);
}

#[cfg(test)]
pub(crate) fn hotkey_action_from_wparam(wparam: WPARAM) -> Option<HotkeyAction> {
    let id = hotkey_id_from_wparam(wparam);
    action_from_id(id)
}

#[cfg(not(test))]
fn hotkey_action_from_wparam(wparam: WPARAM) -> Option<HotkeyAction> {
    let id = hotkey_id_from_wparam(wparam);
    action_from_id(id)
}

fn on_hotkey(hwnd: HWND, wparam: WPARAM) -> LRESULT {
    let _id = hotkey_id_from_wparam(wparam);

    #[cfg(debug_assertions)]
    crate::helpers::debug_log(&format!("WM_HOTKEY id={_id}"));

    let Some(action) = hotkey_action_from_wparam(wparam) else {
        return LRESULT(0);
    };

    with_state_mut(hwnd, |state| match action {
        HotkeyAction::PauseToggle => {
            tracing::warn!(msg = "autoconvert_toggle", source = "hotkey_pause_toggle");
            handle_pause_toggle(hwnd, state)
        }
        HotkeyAction::ConvertLastWord => handle_convert_smart(state),
        HotkeyAction::ConvertSelection => crate::conversion::convert_selection(state),
        HotkeyAction::SwitchLayout => {
            let _ = switch_keyboard_layout();
        }
    });

    LRESULT(0)
}

/// Handles `WM_APP_ERROR` by draining and presenting a single queued UI error.
///
/// Returns `LRESULT(0)` to satisfy the window procedure contract.
fn on_app_error(hwnd: HWND) -> LRESULT {
    with_state_mut_do(hwnd, |state| {
        drain_one_and_present(hwnd, state);
    });

    LRESULT(0)
}

fn on_timer(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    let _ = handle_tray_timer(hwnd, wparam);
    #[cfg(debug_assertions)]
    let _ = handle_timer(hwnd, wparam.0);
    LRESULT(0)
}

fn toggle_window_visibility_from_tray(hwnd: HWND) {
    unsafe {
        let visible = IsWindowVisible(hwnd).as_bool();
        if visible {
            let _ = ShowWindow(hwnd, SW_HIDE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
    }
}

fn set_autoconvert_enabled_from_tray(
    hwnd: HWND,
    state: &mut crate::app::AppState,
    enabled: bool,
    show_balloon: bool,
) {
    if state.autoconvert_enabled == enabled {
        return;
    }

    state.autoconvert_enabled = enabled;

    if let Err(e) = crate::platform::win::tray::switch_tray_icon(hwnd, enabled) {
        tracing::warn!(error = ?e, "switch_tray_icon failed");
    }

    if !show_balloon {
        return;
    }

    let hotkey_text = if state.hotkey_sequence_values.pause.is_some() {
        crate::platform::win::format_hotkey_sequence(state.hotkey_sequence_values.pause)
    } else {
        crate::platform::win::format_hotkey(state.hotkey_values.pause)
    };

    let body = if enabled {
        format!("Status: active.\nAuto convert: ON.\nToggle: {hotkey_text}")
    } else {
        format!("Status: paused.\nAuto convert: OFF.\nToggle: {hotkey_text}")
    };

    if let Err(e) = crate::platform::win::tray::balloon_info(hwnd, "RustSwitcher", &body) {
        tracing::warn!(error = ?e, "tray balloon failed");
    }
}
