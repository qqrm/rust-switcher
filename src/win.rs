//! Window creation and message dispatching.
//!
//! The `win` module encapsulates the main window procedure and the
//! event loop for the application.  It wires together the helper
//! routines, the application state, and the UI construction code to
//! present a settings window and respond to user actions.

use crate::app::{AppState, ID_APPLY, ID_CANCEL, ID_EXIT};
use crate::config;
use crate::helpers;
use crate::ui;
use crate::visuals;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, HFONT, HGDIOBJ};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, BM_SETCHECK, BN_CLICKED, CS_HREDRAW, CS_VREDRAW, CreateWindowExW,
    DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_SHARED, LoadImageW, MSG,
    PostQuitMessage, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, SW_SHOW, SendMessageW,
    SetWindowLongPtrW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_COMMAND, WM_CREATE,
    WM_DESTROY, WM_SETICON, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
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

fn compute_window_size(style: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE) -> (i32, i32) {
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

fn set_window_icons(hwnd: HWND, hinstance: windows::Win32::Foundation::HINSTANCE) {
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

fn apply_config_to_ui(state: &AppState, cfg: &config::Config) {
    unsafe {
        let autostart = if cfg.start_on_startup {
            BST_CHECKED
        } else {
            BST_UNCHECKED
        };
        let tray = if cfg.show_tray_icon {
            BST_CHECKED
        } else {
            BST_UNCHECKED
        };

        let _ = SendMessageW(
            state.chk_autostart,
            BM_SETCHECK,
            Some(WPARAM(autostart.0 as usize)),
            Some(LPARAM(0)),
        );

        let _ = SendMessageW(
            state.chk_tray,
            BM_SETCHECK,
            Some(WPARAM(tray.0 as usize)),
            Some(LPARAM(0)),
        );

        let delay = cfg.delay_ms.to_string();
        let delay_w: Vec<u16> = delay.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
            state.edit_delay_ms,
            PCWSTR(delay_w.as_ptr()),
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
    unsafe {
        match msg {
            WM_CREATE => on_create(hwnd),
            WM_COMMAND => {
                // Decode the command and notification codes.
                let id = helpers::loword(wparam.0);
                let notif = helpers::hiword(wparam.0);
                if u32::from(notif) == BN_CLICKED {
                    match id as i32 {
                        ID_EXIT => {
                            let _ = DestroyWindow(hwnd);
                            return LRESULT(0);
                        }
                        ID_APPLY => {
                            // TODO: read values from controls, update
                            // configuration and apply settings.
                            return LRESULT(0);
                        }
                        ID_CANCEL => {
                            // TODO: revert UI changes or hide the
                            // window when system tray support is
                            // implemented.
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                // Request termination of the message loop.
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                // Clean up application state stored in the window.
                let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
                if !p.is_null() {
                    if !(*p).font.0.is_null() {
                        let _ = DeleteObject(HGDIOBJ((*p).font.0));
                    }
                    drop(Box::from_raw(p));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
