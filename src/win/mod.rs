//! Window creation and message dispatching.
//!
//! The `win` module encapsulates the main window procedure and the
//! event loop for the application.  It wires together the helper
//! routines, the application state, and the UI construction code to
//! present a settings window and respond to user actions.

mod debug_keyboard;

use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{COLOR_WINDOW, DeleteObject, GetSysColorBrush, HFONT, HGDIOBJ},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            AdjustWindowRectEx, BN_CLICKED, CS_HREDRAW, CS_VREDRAW, CreateWindowExW,
            DefWindowProcW, DestroyWindow, DispatchMessageW, EN_KILLFOCUS, EN_SETFOCUS,
            GWLP_USERDATA, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, HICON, ICON_BIG,
            ICON_SMALL, IMAGE_ICON, LR_SHARED, LoadImageW, MSG, PostQuitMessage, SM_CXICON,
            SM_CXSMICON, SM_CYICON, SM_CYSMICON, SW_SHOW, SendMessageW, SetWindowLongPtrW,
            ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_CREATE,
            WM_CTLCOLORBTN, WM_CTLCOLORDLG, WM_CTLCOLORSTATIC, WM_DESTROY, WM_HOTKEY, WM_SETICON,
            WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
        },
    },
    core::{PCWSTR, Result, w},
};

use crate::{
    app::{AppState, ControlId},
    config, helpers,
    hotkeys::{HotkeyAction, action_from_id, register_from_config},
    ui,
    ui::error_notifier::{T_CONFIG, T_UI},
    ui_call, visuals,
};

const WM_APP_ERROR: u32 = crate::ui::error_notifier::WM_APP_ERROR;

fn register_main_class(
    class_name: PCWSTR,
    hinstance: windows::Win32::Foundation::HINSTANCE,
) -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{RegisterClassExW, WNDCLASSEXW};

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        lpszClassName: class_name,
        hInstance: hinstance,
        hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
        ..Default::default()
    };

    unsafe {
        if RegisterClassExW(&wc) == 0 {
            return Err(helpers::last_error());
        }
    }
    Ok(())
}

fn compute_window_size(style: WINDOW_STYLE) -> Result<(i32, i32)> {
    const CLIENT_W: i32 = 760;
    const CLIENT_H: i32 = 230;

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: CLIENT_W,
        bottom: CLIENT_H,
    };

    unsafe {
        AdjustWindowRectEx(&mut rect, style, false, WINDOW_EX_STYLE(0))?;
    }

    Ok((rect.right - rect.left, rect.bottom - rect.top))
}

fn create_main_window(
    class_name: PCWSTR,
    hinstance: windows::Win32::Foundation::HINSTANCE,
    style: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE,
    x: i32,
    y: i32,
    window_w: i32,
    window_h: i32,
) -> Result<HWND> {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("RustSwitcher"),
            style,
            x,
            y,
            window_w,
            window_h,
            None,
            None,
            Some(hinstance),
            None,
        )
    }
}

fn set_window_icons(hwnd: HWND, hinstance: HINSTANCE) {
    unsafe {
        let big = LoadImageW(
            Some(hinstance),
            #[allow(clippy::manual_dangling_ptr)]
            PCWSTR(1usize as *const u16),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXICON),
            GetSystemMetrics(SM_CYICON),
            LR_SHARED,
        )
        .ok()
        .map(|h| HICON(h.0))
        .unwrap_or_default();

        let small = LoadImageW(
            Some(hinstance),
            #[allow(clippy::manual_dangling_ptr)]
            PCWSTR(1usize as *const u16),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_SHARED,
        )
        .ok()
        .map(|h| HICON(h.0))
        .unwrap_or_default();

        if !big.0.is_null() {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_BIG as usize)),
                Some(LPARAM(big.0 as isize)),
            );
        }

        if !small.0.is_null() {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_SMALL as usize)),
                Some(LPARAM(small.0 as isize)),
            );
        }
    }
}

fn message_loop() -> Result<()> {
    unsafe {
        let mut msg = MSG::default();
        loop {
            let r = GetMessageW(&mut msg, None, 0, 0);
            if r.0 == -1 {
                return Err(helpers::last_error());
            }
            if r.0 == 0 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn format_hotkey(hk: Option<config::Hotkey>) -> String {
    let Some(hk) = hk else {
        return "None".to_string();
    };

    let chord = config::HotkeyChord {
        mods: hk.mods,
        vk: (hk.vk != 0).then_some(hk.vk),
    };

    format_hotkey_chord(chord)
}

fn format_hotkey_sequence(seq: Option<config::HotkeySequence>) -> String {
    let Some(seq) = seq else {
        return "None".to_string();
    };

    let mut chords: Vec<String> = Vec::new();
    chords.push(format_hotkey_chord(seq.first));
    if let Some(c1) = seq.second {
        chords.push(format_hotkey_chord(c1));
    }

    chords.join("; ")
}

fn format_hotkey_chord(ch: config::HotkeyChord) -> String {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyNameTextW, MAPVK_VK_TO_VSC, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, MapVirtualKeyW,
    };

    fn vk_to_display(vk: u32) -> String {
        if (0x41..=0x5A).contains(&vk) || (0x30..=0x39).contains(&vk) {
            return (vk as u8 as char).to_string();
        }

        let sc = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) };
        if sc == 0 {
            return format!("VK 0x{:02X}", vk);
        }

        let lparam = ((sc as i32) << 16) as i32;

        let mut buf = [0u16; 64];
        let len = unsafe { GetKeyNameTextW(lparam, &mut buf) };
        if len <= 0 {
            return format!("VK 0x{:02X}", vk);
        }

        String::from_utf16_lossy(&buf[..(len as usize)])
    }

    let mut parts: Vec<String> = Vec::new();

    if (ch.mods & MOD_CONTROL.0) != 0 {
        parts.push("Ctrl".to_string());
    }
    if (ch.mods & MOD_ALT.0) != 0 {
        parts.push("Alt".to_string());
    }
    if (ch.mods & MOD_SHIFT.0) != 0 {
        parts.push("Shift".to_string());
    }
    if (ch.mods & MOD_WIN.0) != 0 {
        parts.push("Win".to_string());
    }

    if let Some(vk) = ch.vk {
        parts.push(vk_to_display(vk));
    }

    if parts.is_empty() {
        "None".to_string()
    } else {
        parts.join(" + ")
    }
}

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

    cfg.hotkey_convert_last_word = state.hotkey_values.last_word;
    cfg.hotkey_pause = state.hotkey_values.pause;
    cfg.hotkey_convert_selection = state.hotkey_values.selection;
    cfg.hotkey_switch_layout = state.hotkey_values.switch_layout;

    cfg.hotkey_convert_last_word_sequence = state.hotkey_sequence_values.last_word;
    cfg.hotkey_pause_sequence = state.hotkey_sequence_values.pause;
    cfg.hotkey_convert_selection_sequence = state.hotkey_sequence_values.selection;
    cfg.hotkey_switch_layout_sequence = state.hotkey_sequence_values.switch_layout;

    cfg
}

fn apply_config_runtime(
    hwnd: HWND,
    state: &mut AppState,
    cfg: &config::Config,
) -> windows::core::Result<()> {
    state.paused = cfg.paused;

    let _ = crate::hotkeys::register_from_config(hwnd, cfg);

    // TODO: включить или выключить трей
    // TODO: включить или выключить автозапуск
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

fn on_create(hwnd: HWND) -> LRESULT {
    let mut state = Box::new(AppState::default());

    #[rustfmt::skip]
    startup_or_return0!(hwnd, &mut state, "Failed to create UI controls", ui::create_controls(hwnd, &mut state));

    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            use windows::core::{Error, HRESULT};
            let we = Error::new(HRESULT(0x80004005u32 as i32), e.to_string());
            crate::ui::error_notifier::push(
                hwnd,
                &mut state,
                "",
                "Failed to load config, falling back to defaults",
                &we,
            );
            on_app_error(hwnd);
            config::Config::default()
        }
    };

    state.hotkey_values = crate::app::HotkeyValues::from_config(&cfg);
    #[rustfmt::skip]
    startup_or_return0!(hwnd, &mut state, "Failed to apply config to UI", apply_config_to_ui(state.as_mut(), &cfg));

    #[rustfmt::skip]
    startup_or_return0!(hwnd, &mut state, "Failed to register hotkeys", register_from_config(hwnd, &cfg));

    debug_keyboard::install(hwnd);

    init_font_and_visuals(hwnd, &mut state);

    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
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
        WM_COMMAND => on_command(hwnd, wparam, lparam),
        WM_HOTKEY => on_hotkey(hwnd, wparam, lparam),

        WM_CTLCOLORDLG | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            crate::ui::colors::on_ctlcolor(wparam, lparam)
        }

        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }

        WM_NCDESTROY => unsafe { on_ncdestroy(hwnd) },

        WM_APP_ERROR => on_app_error(hwnd),

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn io_to_win(e: std::io::Error) -> windows::core::Error {
    use windows::core::{Error, HRESULT};
    Error::new(HRESULT(0x80004005u32 as i32), e.to_string())
}

fn handle_apply(hwnd: HWND, state: &mut AppState) {
    let base = match config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            let e = io_to_win(e);
            crate::ui::error_notifier::push(
                hwnd,
                state,
                "",
                "Failed to load config before applying changes",
                &e,
            );
            on_app_error(hwnd);
            config::Config::default()
        }
    };

    let cfg = read_ui_to_config(state, base);

    if let Err(e) = config::save(&cfg) {
        let e = io_to_win(e);
        crate::ui::error_notifier::push(hwnd, state, "", "Failed to save config", &e);
        on_app_error(hwnd);
        return;
    }

    #[rustfmt::skip]
    ui_call!(hwnd, state, T_CONFIG, "Failed to apply config at runtime", apply_config_runtime(hwnd, state, &cfg));

    #[rustfmt::skip]
    ui_call!(hwnd, state, T_UI, "Failed to update UI from config", apply_config_to_ui(state, &cfg));
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

#[cfg_attr(
    debug_assertions,
    tracing::instrument(level = "info", skip_all, fields(msg, id, notif))
)]
fn on_command(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    #[cfg(debug_assertions)]
    tracing::Span::current().record("msg", "WM_COMMAND");

    let id = crate::helpers::loword(wparam.0) as i32;
    let notif = crate::helpers::hiword(wparam.0) as u32;

    #[cfg(debug_assertions)]
    {
        tracing::Span::current().record("id", id);
        tracing::Span::current().record("notif", notif as i64);
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

                #[cfg(debug_assertions)]
                eprintln!("hotkey.capture: start slot={:?}", slot);
            });
            Some(LRESULT(0))
        }
        EN_KILLFOCUS => {
            with_state_mut_do(hwnd, |state| {
                state.hotkey_capture.active = false;

                #[cfg(debug_assertions)]
                eprintln!("hotkey.capture: stop slot={:?}", slot);
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
                crate::ui::error_notifier::push(
                    hwnd,
                    state,
                    T_UI,
                    "Failed to close the window",
                    &e,
                );
                on_app_error(hwnd);
            }
        }),
        ControlId::Apply => with_state_mut_do(hwnd, |state| handle_apply(hwnd, state)),
        ControlId::Cancel => with_state_mut_do(hwnd, |state| handle_cancel(hwnd, state)),
        _ => {}
    }

    LRESULT(0)
}

unsafe fn on_ncdestroy(hwnd: HWND) -> LRESULT {
    #[cfg(debug_assertions)]
    debug_keyboard::uninstall();

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

fn with_state_mut<R>(hwnd: HWND, f: impl FnOnce(&mut AppState) -> R) -> Option<R> {
    unsafe {
        let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
        (!p.is_null()).then(|| f(&mut *p))
    }
}

fn with_state_mut_do(hwnd: HWND, f: impl FnOnce(&mut AppState)) {
    unsafe {
        let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
        if !p.is_null() {
            f(&mut *p);
        }
    }
}
fn on_hotkey(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    let id = wparam.0 as i32;

    #[cfg(debug_assertions)]
    crate::helpers::debug_log(&format!("WM_HOTKEY id={}", id));

    let Some(action) = action_from_id(id) else {
        return LRESULT(0);
    };

    with_state_mut(hwnd, |state| match action {
        HotkeyAction::PauseToggle => {
            state.paused = !state.paused;
        }
        HotkeyAction::ConvertLastWord => {
            if !state.paused {
                crate::conversion::convert_last_word(state, hwnd);
            }
        }
        HotkeyAction::ConvertSelection => {
            if !state.paused {
                crate::conversion::convert_selection(state, hwnd);
            }
        }
        HotkeyAction::SwitchLayout => {
            if !state.paused {
                let _ = crate::conversion::switch_keyboard_layout();
            }
        }
    });

    LRESULT(0)
}

fn on_app_error(hwnd: HWND) -> LRESULT {
    with_state_mut(hwnd, |state| {
        if let Some(e) = crate::ui::error_notifier::drain_one(state) {
            if let Err(te) = crate::tray::balloon_error(hwnd, &e.title, &e.user_text) {
                #[cfg(debug_assertions)]
                eprintln!("tray balloon failed: {:?}", te);
            }

            #[cfg(debug_assertions)]
            eprintln!("{}: {}", e.title, e.debug_text);
        }
    });
    LRESULT(0)
}
