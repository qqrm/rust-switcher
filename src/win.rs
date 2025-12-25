//! Window creation and message dispatching.
//!
//! The `win` module encapsulates the main window procedure and the
//! event loop for the application.  It wires together the helper
//! routines, the application state, and the UI construction code to
//! present a settings window and respond to user actions.

mod commands;
mod hotkey_format;
pub(crate) mod keyboard;
pub(crate) mod mouse;
mod state;
mod window;

pub(crate) use hotkey_format::{format_hotkey, format_hotkey_sequence};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{DeleteObject, HFONT, HGDIOBJ},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DefWindowProcW, GetWindowLongPtrW, PostQuitMessage, SetWindowLongPtrW, ShowWindow,
            SW_SHOW, GWLP_USERDATA,
            WM_COMMAND, WM_CREATE, 
            WM_CTLCOLORBTN, WM_CTLCOLORDLG,
            WM_CTLCOLORSTATIC, WM_DESTROY, 
            WM_HOTKEY, WM_TIMER, WS_MAXIMIZEBOX,
            WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
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
use crate::{
    tray::WM_APP_TRAY,
    app::AppState,
    config, helpers,
    hotkeys::{HotkeyAction, action_from_id, register_from_config},
    ui::{
        self,
        error_notifier::{T_CONFIG, T_UI, drain_one_and_present},
    },
    ui_call, ui_try, visuals,
};

fn set_hwnd_text(hwnd: HWND, s: &str) -> windows::core::Result<()> {
    helpers::set_edit_text(hwnd, s)
}

fn apply_config_to_ui(state: &mut AppState, cfg: &config::Config) -> windows::core::Result<()> {
    helpers::set_checkbox(state.checkboxes.autostart, cfg.start_on_startup);
    helpers::set_checkbox(state.checkboxes.tray, cfg.show_tray_icon);
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
    cfg.start_on_startup = helpers::get_checkbox(state.checkboxes.autostart);
    cfg.show_tray_icon = helpers::get_checkbox(state.checkboxes.tray);
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
        Some(_) => None, // важно: только LL hook, иначе Windows не отличит LShift от RShift
        None => state.hotkey_values.switch_layout,
    };

    cfg
}

fn apply_config_runtime(
    hwnd: HWND,
    state: &mut AppState,
    cfg: &config::Config,
) -> windows::core::Result<()> {
    state.paused = cfg.paused;

    // Critical: refresh runtime hotkey matcher inputs
    state.active_hotkey_sequences = crate::app::HotkeySequenceValues::from_config(cfg);

    // Reset runtime progress to avoid stale "waiting_second" and pending-mod state
    state.runtime_chord_capture = crate::app::RuntimeChordCapture::default();
    state.hotkey_sequence_progress = crate::app::HotkeySequenceProgress::default();

    // Legacy fields (можно будет удалить позже, сейчас оставляем чтобы не ломать контракт)
    state.active_switch_layout_sequence = cfg.hotkey_switch_layout_sequence;
    state.switch_layout_waiting_second = false;
    state.switch_layout_first_tick_ms = 0;

    // Registering system hotkeys can legitimately fail for modifier-only bindings
    // so report but do not fail the whole apply.
    ui_try!(
        hwnd,
        state,
        T_CONFIG,
        "Failed to register hotkeys",
        crate::hotkeys::register_from_config(hwnd, cfg)
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
                $crate::ui::error_notifier::push($hwnd, $state, "", $text, &e);
                on_app_error($hwnd);
                return LRESULT(0);
            }
        }
    }};
}

/// Loads config and validates it for runtime use.
///
/// On invalid config, this function notifies the user and falls back to defaults.
/// This keeps the application operational even when the config file was edited manually.
fn load_config_or_default(hwnd: HWND, state: &mut AppState) -> config::Config {
    config::load()
        .map_err(|e| {
            crate::ui::error_notifier::push(
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
                crate::ui::error_notifier::push(hwnd, state, T_CONFIG, &user_text, &source);
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

    #[rustfmt::skip]
    startup_or_return0!(hwnd, &mut state, "Failed to apply config to UI", apply_config_to_ui(state.as_mut(), &cfg));

    #[rustfmt::skip]
    startup_or_return0!(hwnd, &mut state, "Failed to register hotkeys", register_from_config(hwnd, &cfg));

    keyboard::install(hwnd, state.as_mut());
    mouse::install();

    init_font_and_visuals(hwnd, &mut state);

    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    }

    #[cfg(debug_assertions)]
    with_state_mut_do(hwnd, |state| {
        use std::{thread::sleep, time};

        helpers::debug_startup_notification(hwnd, state);
        sleep(time::Duration::from_millis(2000));
        helpers::debug_startup_notification(hwnd, state);
    });

    LRESULT(0)
}

/// Start the main window and enter the message loop.
///
/// This function is called from `main` after the single instance
/// guard has been acquired.  It performs all initialization that
/// requires unsafe code and returns any error to the caller.
pub fn run() -> Result<()> {
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
        let _ = ShowWindow(hwnd, SW_SHOW);

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
        WM_COMMAND => commands::on_command(hwnd, wparam, lparam),
        WM_HOTKEY => on_hotkey(hwnd, wparam),
        WM_TIMER => on_timer(hwnd, wparam),
        WM_CTLCOLORDLG | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            crate::ui::colors::on_ctlcolor(wparam, lparam)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_NCDESTROY => unsafe { on_ncdestroy(hwnd) },
        crate::ui::error_notifier::WM_APP_AUTOCONVERT => {
            if crate::input_journal::last_token_autoconverted() {
                return LRESULT(0);
            }

            with_state_mut_do(hwnd, |state| {
                if !state.paused {
                    crate::conversion::last_word::autoconvert_last_word(state);
                }
            });

            LRESULT(0)
        }
        WM_APP_TRAY => {
            let lparam_val = lparam.0 as u32;
            
            match lparam_val {
                0x1007b => {
                    println!("WM_CONTEXTMENU - SHOWING MENU");
                    let _ = crate::tray::show_tray_context_menu(hwnd);
                }
                _ => return LRESULT(0),
            }
            
            return LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn io_to_win(e: std::io::Error) -> windows::core::Error {
    use windows::core::{Error, HRESULT};
    Error::new(HRESULT(0x80004005u32 as i32), e.to_string())
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
        crate::ui::error_notifier::push(hwnd, state, T_CONFIG, &self.user_text, &self.source);
    }
}

/// Builds a config from the current UI state and persists it.
///
/// Responsibilities:
/// - loads the current config as a base (so unspecified fields are preserved)
/// - reads UI controls into a new config instance
/// - saves the config to persistent storage
///
/// Error semantics:
/// Returns an `ApplyConfigError` that is suitable for direct user notification.
/// In particular, validation failures are reported with the validation message as `user_text`.
fn build_and_save_config_from_ui(
    state: &mut AppState,
) -> std::result::Result<config::Config, ApplyConfigError> {
    let base = config::load().map_err(|e| ApplyConfigError {
        user_text: "Failed to load config before applying changes".to_string(),
        source: io_to_win(e),
    })?;

    let cfg = read_ui_to_config(state, base);

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
    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            let e = io_to_win(e);
            crate::ui::error_notifier::push(hwnd, state, "", "Failed to load config", &e);
            on_app_error(hwnd);
            return;
        }
    };

    #[rustfmt::skip]
    ui_call!(hwnd, state, T_CONFIG, "Failed to apply config at runtime", apply_config_runtime(hwnd, state, &cfg));
    #[rustfmt::skip]
    ui_call!(hwnd, state, T_UI, "Failed to update UI from config", apply_config_to_ui(state, &cfg));
}

unsafe fn on_ncdestroy(hwnd: HWND) -> LRESULT {
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
    if p.is_null() {
        return LRESULT(0);
    }

    crate::tray::remove_icon(hwnd);

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
    state.paused = !state.paused;

    let hotkey_text = if state.hotkey_sequence_values.pause.is_some() {
        format_hotkey_sequence(state.hotkey_sequence_values.pause)
    } else {
        format_hotkey(state.hotkey_values.pause)
    };

    let body = if state.paused {
        format!(
            "Статус: деактивирована.\nАвтоконвертация выключена.\nПереключить: {}",
            hotkey_text
        )
    } else {
        format!(
            "Статус: активирована.\nАвтоконвертация включена.\nПереключить: {}",
            hotkey_text
        )
    };

    if let Err(_e) = crate::tray::balloon_info(hwnd, "RustSwitcher", &body) {
        #[cfg(debug_assertions)]
        eprintln!("tray balloon failed: {:?}", _e);
    }
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
    crate::helpers::debug_log(&format!("WM_HOTKEY id={}", _id));

    let Some(action) = hotkey_action_from_wparam(wparam) else {
        return LRESULT(0);
    };

    with_state_mut(hwnd, |state| match action {
        HotkeyAction::PauseToggle => handle_pause_toggle(hwnd, state),
        HotkeyAction::ConvertLastWord => handle_convert_smart(state),
        HotkeyAction::ConvertSelection => crate::conversion::convert_selection(state),
        HotkeyAction::SwitchLayout => {
            let _ = crate::conversion::switch_keyboard_layout();
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

fn on_timer(_hwnd: HWND, _wparam: WPARAM) -> LRESULT {
    #[cfg(debug_assertions)]
    {
        use crate::win::keyboard::debug_timers::handle_timer;

        if let Some(r) = handle_timer(_hwnd, _wparam.0) {
            return r;
        }
    }

    LRESULT(0)
}
