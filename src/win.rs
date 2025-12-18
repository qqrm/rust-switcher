//! Window creation and message dispatching.
//!
//! The `win` module encapsulates the main window procedure and the
//! event loop for the application.  It wires together the helper
//! routines, the application state, and the UI construction code to
//! present a settings window and respond to user actions.

use crate::app::AppState;
use crate::config;
use crate::helpers;
use crate::ui;
use crate::visuals;

use crate::app::ControlId;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, HFONT, HGDIOBJ};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::MOD_ALT;
use windows::Win32::UI::Input::KeyboardAndMouse::MOD_CONTROL;
use windows::Win32::UI::Input::KeyboardAndMouse::MOD_SHIFT;
use windows::Win32::UI::Input::KeyboardAndMouse::MOD_WIN;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CANCEL, VK_PAUSE};
use windows::Win32::UI::WindowsAndMessaging::WM_HOTKEY;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, BN_CLICKED, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_SHARED, LoadImageW, MSG,
    PostQuitMessage, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, SW_SHOW, SendMessageW,
    SetWindowLongPtrW, SetWindowTextW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_COMMAND, WM_CREATE, WM_DESTROY, WM_SETICON, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW,
    WS_THICKFRAME,
};
use windows::core::{PCWSTR, Result, w};

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
        ..Default::default()
    };

    unsafe {
        if RegisterClassExW(&wc) == 0 {
            return Err(helpers::last_error());
        }
    }
    Ok(())
}

fn compute_window_size(style: WINDOW_STYLE) -> (i32, i32) {
    const CLIENT_W: i32 = 540;
    const CLIENT_H: i32 = 230;

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: CLIENT_W,
        bottom: CLIENT_H,
    };

    unsafe {
        let _ = AdjustWindowRectEx(&mut rect, style, false, WINDOW_EX_STYLE(0));
    }

    let window_w = rect.right - rect.left;
    let window_h = rect.bottom - rect.top;
    (window_w, window_h)
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
            PCWSTR(1usize as *const u16),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_SHARED,
        )
        .ok()
        .map(|h| HICON(h.0))
        .unwrap_or_default();

        if big.0 != std::ptr::null_mut() {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_BIG as usize)),
                Some(LPARAM(big.0 as isize)),
            );
        }

        if small.0 != std::ptr::null_mut() {
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

    const MODS: &[(u32, &str)] = &[
        (MOD_CONTROL.0, "Ctrl"),
        (MOD_ALT.0, "Alt"),
        (MOD_SHIFT.0, "Shift"),
        (MOD_WIN.0, "Win"),
    ];

    let parts: Vec<&str> = MODS
        .iter()
        .filter_map(|&(m, s)| ((hk.mods & m) != 0).then_some(s))
        .collect();

    let key = match hk.vk as u16 {
        v if v == VK_PAUSE.0 => "Pause".to_string(),
        v if v == VK_CANCEL.0 => "Cancel".to_string(),
        _ => format!("VK 0x{:02X}", hk.vk),
    };

    parts
        .into_iter()
        .map(str::to_string)
        .chain(std::iter::once(key))
        .collect::<Vec<_>>()
        .join(" + ")
}

fn set_hwnd_text(hwnd: HWND, s: &str) {
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    wide.push(0);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
    }
}

fn apply_config_to_ui(state: &AppState, cfg: &config::Config) {
    unsafe {
        helpers::set_checkbox(state.checkboxes.autostart, cfg.start_on_startup);
        helpers::set_checkbox(state.checkboxes.tray, cfg.show_tray_icon);
        helpers::set_edit_u32(state.edits.delay_ms, cfg.delay_ms);

        set_hwnd_text(
            state.hotkeys.last_word,
            &format_hotkey(cfg.hotkey_convert_last_word),
        );
        set_hwnd_text(state.hotkeys.pause, &format_hotkey(cfg.hotkey_pause));
        set_hwnd_text(
            state.hotkeys.selection,
            &format_hotkey(cfg.hotkey_convert_selection),
        );
        set_hwnd_text(
            state.hotkeys.switch_layout,
            &format_hotkey(cfg.hotkey_switch_layout),
        );
    }
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

fn on_create(hwnd: HWND) -> LRESULT {
    unsafe {
        let mut state = Box::new(AppState::default());

        if ui::create_controls(hwnd, &mut state).is_err() {
            let _ = DestroyWindow(hwnd);
            return LRESULT(0);
        }

        let cfg = config::load().unwrap_or_default();
        apply_config_to_ui(&state, &cfg);

        init_font_and_visuals(hwnd, &mut state);

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
        LRESULT(0)
    }
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
        let (window_w, window_h) = compute_window_size(style);
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
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_NCDESTROY => unsafe { on_ncdestroy(hwnd) },
        WM_HOTKEY => on_hotkey(hwnd, wparam, lparam),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn on_command(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    let id = helpers::loword(wparam.0) as i32;
    let notif = helpers::hiword(wparam.0);

    if u32::from(notif) != BN_CLICKED {
        return LRESULT(0);
    }

    let Some(cid) = ControlId::from_i32(id) else {
        return LRESULT(0);
    };

    match cid {
        ControlId::Exit => unsafe {
            let _ = DestroyWindow(hwnd);
        },
        ControlId::Apply => {
            // TODO
        }
        ControlId::Cancel => {
            let _ = with_state_mut(hwnd, |state| {
                if let Ok(cfg) = config::load() {
                    apply_config_to_ui(state, &cfg);
                }
            });
        }
        _ => {}
    }

    LRESULT(0)
}

unsafe fn on_ncdestroy(hwnd: HWND) -> LRESULT {
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
    if p.is_null() {
        return LRESULT(0);
    }

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

fn on_hotkey(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    let id = wparam.0 as i32;

    let Some(action) = crate::hotkeys::action_from_id(id) else {
        return LRESULT(0);
    };

    match action {
        crate::hotkeys::HotkeyAction::PauseToggle => {
            with_state_mut(hwnd, |state| {
                state.paused = !state.paused;
            });
        }
        crate::hotkeys::HotkeyAction::ConvertLastWord => {
            // TODO
        }
        crate::hotkeys::HotkeyAction::ConvertSelection => {
            // TODO
        }
        crate::hotkeys::HotkeyAction::SwitchLayout => {
            // TODO
        }
    }

    LRESULT(0)
}
