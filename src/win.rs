//! Window creation and message dispatching.
//!
//! The `win` module encapsulates the main window procedure and the
//! event loop for the application.  It wires together the helper
//! routines, the application state, and the UI construction code to
//! present a settings window and respond to user actions.

use crate::app::{AppState, ID_APPLY, ID_CANCEL, ID_EXIT};
use crate::helpers;
use crate::ui;
use crate::visuals;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, HFONT, HGDIOBJ};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, BN_CLICKED, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED,
    LoadImageW, MSG, PostQuitMessage, RegisterClassExW, SM_CXICON, SM_CXSCREEN, SM_CXSMICON,
    SM_CYICON, SM_CYSCREEN, SM_CYSMICON, SW_SHOW, SendMessageW, SetWindowLongPtrW, ShowWindow,
    TranslateMessage, WINDOW_EX_STYLE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_SETICON, WNDCLASSEXW,
    WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
};
use windows::core::{PCWSTR, Result, w};

/// Start the main window and enter the message loop.
///
/// This function is called from `main` after the single instance
/// guard has been acquired.  It performs all initialization that
/// requires unsafe code and returns any error to the caller.
pub fn run() -> Result<()> {
    unsafe {
        // Initialise common controls so that group boxes and other
        // controls use the modern visual style.
        visuals::init_visuals();

        let class_name = w!("RustSwitcherMainWindow");
        let hinstance = GetModuleHandleW(PCWSTR::null())?.into();

        let icon = LoadImageW(
            Some(hinstance),
            PCWSTR(1usize as *const u16),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
        .ok()
        .map(|h| HICON(h.0))
        .unwrap_or_default();

        // Define the window class with our custom window procedure.
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            lpszClassName: class_name,
            hInstance: hinstance,
            hIcon: icon,
            hIconSm: icon,
            ..Default::default()
        };

        if RegisterClassExW(&wc) == 0 {
            return Err(helpers::last_error());
        }

        // Compute the outer dimensions of the window based on the
        // desired client size.  The original project hard‑codes a
        // 540×230 client area.
        const CLIENT_W: i32 = 540;
        const CLIENT_H: i32 = 230;
        let style = WS_OVERLAPPEDWINDOW & !WS_THICKFRAME & !WS_MAXIMIZEBOX;
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: CLIENT_W,
            bottom: CLIENT_H,
        };
        let _ = AdjustWindowRectEx(&mut rect, style, false, WINDOW_EX_STYLE(0));
        let window_w = rect.right - rect.left;
        let window_h = rect.bottom - rect.top;

        // Centre the window on the primary monitor.
        let x = (GetSystemMetrics(SM_CXSCREEN) - window_w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - window_h) / 2;

        // Create the main application window.  The window title is
        // provided via the `w!` macro.  No menu is attached, and
        // additional extended styles are not used here.
        let hwnd = CreateWindowExW(
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
        )?;

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

        // Show the window after creation.  Without this call it will
        // remain hidden.
        let _ = ShowWindow(hwnd, SW_SHOW);

        // Standard Windows message loop.  Continues until a
        // WM_QUIT message is posted.
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

/// The window procedure.  Handles creation, command and destroy
/// messages.  Any unhandled messages are forwarded to the default
/// procedure.
pub extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    const WM_NCDESTROY: u32 = 0x0082;
    unsafe {
        match msg {
            WM_CREATE => {
                // Allocate application state and build controls.
                let mut state = Box::new(AppState::default());
                if let Err(_) = ui::create_controls(hwnd, &mut state) {
                    let _ = DestroyWindow(hwnd);
                    return LRESULT(0);
                }
                // Create a font to use for all controls.  If font
                // creation fails we fall back to a default HFONT.
                match visuals::create_message_font() {
                    Ok(font) => state.font = font,
                    Err(_) => state.font = HFONT::default(),
                }
                if !state.font.0.is_null() {
                    visuals::apply_modern_look(hwnd, state.font);
                }
                // Store the state pointer in the window user data so
                // subsequent messages can retrieve it.
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                LRESULT(0)
            }
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
