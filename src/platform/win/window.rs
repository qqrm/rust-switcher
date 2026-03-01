use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, RECT, WPARAM},
        Graphics::Gdi::{COLOR_WINDOW, GetSysColorBrush},
        UI::WindowsAndMessaging::{
            AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DispatchMessageW,
            GetMessageW, GetSystemMetrics, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_SHARED,
            LoadImageW, MSG, RegisterClassExW, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON,
            SendMessageW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_SETICON, WNDCLASSEXW,
        },
    },
    core::{PCWSTR, Result, w},
};

use super::winutil::make_int_resource;
use crate::utils::helpers;

pub(crate) fn register_main_class(class_name: PCWSTR, hinstance: HINSTANCE) -> Result<()> {
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(super::wndproc),
        lpszClassName: class_name,
        hInstance: hinstance,
        hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
        ..Default::default()
    };

    unsafe {
        if RegisterClassExW(&raw const wc) == 0 {
            return Err(helpers::last_error());
        }
    }
    Ok(())
}

pub(crate) fn compute_window_size(style: WINDOW_STYLE) -> Result<(i32, i32)> {
    const CLIENT_W: i32 = 760;
    const CLIENT_H: i32 = 230;

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: CLIENT_W,
        bottom: CLIENT_H,
    };

    unsafe { AdjustWindowRectEx(&raw mut rect, style, false, WINDOW_EX_STYLE(0))? };

    Ok((rect.right - rect.left, rect.bottom - rect.top))
}

pub(crate) fn create_main_window(
    class_name: PCWSTR,
    hinstance: HINSTANCE,
    style: WINDOW_STYLE,
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

pub(crate) fn set_window_icons(hwnd: HWND, hinstance: HINSTANCE) {
    unsafe {
        let big = LoadImageW(
            Some(hinstance),
            make_int_resource(1),
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
            make_int_resource(1),
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

pub(crate) fn message_loop() -> Result<()> {
    unsafe {
        let mut msg = MSG::default();
        loop {
            let r = GetMessageW(&raw mut msg, None, 0, 0);
            if r.0 == -1 {
                return Err(helpers::last_error());
            }
            if r.0 == 0 {
                break;
            }
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
    Ok(())
}
