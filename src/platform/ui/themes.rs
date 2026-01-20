//! Theme management for Windows UI
//!
//! This module provides theme-aware painting functions for Windows controls
//! and handles dark/light theme switching.
//!
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::{
            Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
            Gdi::{
                COLOR_WINDOW, COLOR_WINDOWTEXT, CreateSolidBrush, DT_CENTER, DT_SINGLELINE,
                DT_VCENTER, DeleteObject, DrawFocusRect, DrawTextW, FillRect, FrameRect,
                GetStockObject, GetSysColor, GetSysColorBrush, HBRUSH, HDC, HGDIOBJ,
                InvalidateRect, NULL_BRUSH, RDW_ALLCHILDREN, RDW_INVALIDATE, RedrawWindow,
                SetBkColor, SetBkMode, SetTextColor, TRANSPARENT, UpdateWindow,
            },
        },
        UI::{
            Controls::{
                DRAWITEMSTRUCT, ODS_DEFAULT, ODS_DISABLED, ODS_FOCUS, ODS_SELECTED, ODT_BUTTON,
                SetWindowTheme,
            },
            WindowsAndMessaging::{GetClientRect, *},
        },
    },
    core::{BOOL, w},
};

use crate::platform::win::state::{get_state, with_state_mut_do};

const DARK_WINDOW_BG: COLORREF = COLORREF(0x002D2D30);
const DARK_CONTROL_BG: COLORREF = COLORREF(0x002D2D30);
const DARK_EDIT_BG: COLORREF = COLORREF(0x001E1E1E);

fn ensure_dark_brushes(hwnd: HWND) {
    with_state_mut_do(hwnd, |state| unsafe {
        if state.dark_brush_window_bg.0.is_null() {
            state.dark_brush_window_bg = CreateSolidBrush(DARK_WINDOW_BG);
        }
        if state.dark_brush_control_bg.0.is_null() {
            state.dark_brush_control_bg = CreateSolidBrush(DARK_CONTROL_BG);
        }
        if state.dark_brush_edit_bg.0.is_null() {
            state.dark_brush_edit_bg = CreateSolidBrush(DARK_EDIT_BG);
        }
    });
}

struct ButtonState {
    pressed: bool,
    focused: bool,
    defaulted: bool,
    disabled: bool,
}

struct ButtonPalette {
    bg_normal: COLORREF,
    bg_pressed: COLORREF,
    bg_focused: COLORREF,
    bg_disabled: COLORREF,

    text_normal: COLORREF,
    text_disabled: COLORREF,

    border_normal: COLORREF,
    border_pressed: COLORREF,
    border_disabled: COLORREF,
}

fn read_button_state(draw_item: &DRAWITEMSTRUCT) -> ButtonState {
    ButtonState {
        pressed: (draw_item.itemState.0 & ODS_SELECTED.0) != 0,
        focused: (draw_item.itemState.0 & ODS_FOCUS.0) != 0,
        defaulted: (draw_item.itemState.0 & ODS_DEFAULT.0) != 0,
        disabled: (draw_item.itemState.0 & ODS_DISABLED.0) != 0,
    }
}

fn read_button_text(hwnd_btn: HWND) -> ([u16; 256], usize) {
    let mut buf = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd_btn, &mut buf) as usize };
    (buf, len)
}

fn choose_colors(state: &ButtonState, palette: &ButtonPalette) -> (COLORREF, COLORREF, COLORREF) {
    let bg = if state.disabled {
        palette.bg_disabled
    } else if state.pressed {
        palette.bg_pressed
    } else if state.focused || state.defaulted {
        palette.bg_focused
    } else {
        palette.bg_normal
    };

    let text = if state.disabled {
        palette.text_disabled
    } else {
        palette.text_normal
    };

    let border = if state.disabled {
        palette.border_disabled
    } else if state.pressed {
        palette.border_pressed
    } else {
        palette.border_normal
    };

    (bg, text, border)
}

fn paint_button_background(hdc: HDC, rect: &RECT, bg: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(bg);
        FillRect(hdc, rect, brush);
        let _ = DeleteObject(HGDIOBJ::from(brush));
    }
}

fn paint_button_border(hdc: HDC, rect: &RECT, border: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(border);
        FrameRect(hdc, rect, brush);
        let _ = DeleteObject(HGDIOBJ::from(brush));
    }
}

fn paint_button_focus(hdc: HDC, rect: &RECT) {
    unsafe {
        let focus_rect = RECT {
            left: rect.left + 3,
            top: rect.top + 3,
            right: rect.right - 3,
            bottom: rect.bottom - 3,
        };
        let _ = DrawFocusRect(hdc, &focus_rect);
    }
}

fn paint_button_text(
    hdc: HDC,
    rect: &RECT,
    text: &mut [u16],
    text_len: usize,
    text_color: COLORREF,
    pressed: bool,
) {
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, text_color);

        let mut text_rect = *rect;
        if pressed {
            text_rect.left += 2;
            text_rect.top += 2;
            text_rect.right += 2;
            text_rect.bottom += 2;
        }

        DrawTextW(
            hdc,
            &mut text[..text_len],
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

fn paint_button(
    hdc: HDC,
    rect: &RECT,
    hwnd_btn: HWND,
    state: &ButtonState,
    palette: &ButtonPalette,
) {
    let (mut text_buf, text_len) = read_button_text(hwnd_btn);
    let (bg, text, border) = choose_colors(state, palette);

    paint_button_background(hdc, rect, bg);

    if state.focused && !state.pressed && !state.disabled {
        paint_button_focus(hdc, rect);
    }

    paint_button_border(hdc, rect, border);
    paint_button_text(
        hdc,
        rect,
        &mut text_buf,
        text_len,
        text,
        state.pressed && !state.disabled,
    );
}

fn palette_dark() -> ButtonPalette {
    ButtonPalette {
        bg_normal: COLORREF(0x00262626),
        bg_pressed: COLORREF(0x003C3C3C),
        bg_focused: COLORREF(0x00323232),
        bg_disabled: COLORREF(0x00333333),

        text_normal: COLORREF(0x00FFFFFF),
        text_disabled: COLORREF(0x00888888),

        border_normal: COLORREF(0x00404040),
        border_pressed: COLORREF(0x00505050),
        border_disabled: COLORREF(0x00404040),
    }
}

fn palette_light() -> ButtonPalette {
    ButtonPalette {
        bg_normal: COLORREF(0x00F0F0F0),
        bg_pressed: COLORREF(0x00C0C0C0),
        bg_focused: COLORREF(0x00E0E0E0),
        bg_disabled: COLORREF(0x00C0C0C0),

        text_normal: COLORREF(0x00000000),
        text_disabled: COLORREF(0x00808080),

        border_normal: COLORREF(0x00808080),
        border_pressed: COLORREF(0x00808080),
        border_disabled: COLORREF(0x00808080),
    }
}

/// Handles `WM_DRAWITEM` messages for owner-drawn buttons.
pub fn on_draw_item(_hwnd: HWND, _wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let draw_item = &*(lparam.0 as *const DRAWITEMSTRUCT);

        if draw_item.CtlType != ODT_BUTTON {
            return LRESULT(0);
        }

        let parent_hwnd = match GetParent(draw_item.hwndItem) {
            Ok(hwnd) => hwnd,
            Err(_) => return LRESULT(0),
        };

        let theme_dark = get_state(parent_hwnd)
            .map(|s| s.current_theme_dark)
            .unwrap_or(false);

        let palette = if theme_dark {
            palette_dark()
        } else {
            palette_light()
        };

        let state = read_button_state(draw_item);

        paint_button(
            draw_item.hDC,
            &draw_item.rcItem,
            draw_item.hwndItem,
            &state,
            &palette,
        );

        LRESULT(1)
    }
}

/// Handles `WM_CTLCOLOR*` style messages by configuring the device context and returning a brush.
/// Expected usage:
/// - called from a window procedure when processing control color messages
/// - `wparam` is interpreted as an `HDC` for the control being painted
///
/// What it does:
/// - sets the text color to the system `COLOR_WINDOWTEXT` color
/// - sets background mode to `TRANSPARENT` so the parent background shows through
/// - returns a system brush for `COLOR_WINDOW` to paint the control background
///
/// Return value:
/// - `LRESULT` containing an `HBRUSH` handle, as required by `WM_CTLCOLOR*`
///
/// Safety:
/// - this function performs raw handle casts (`WPARAM` to `HDC`) and calls Win32 APIs
///   that assume a valid device context.
pub fn on_ctlcolor(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let hdc = HDC(wparam.0 as *mut core::ffi::c_void);

        if let Some(state) = get_state(hwnd)
            && state.current_theme_dark
        {
            ensure_dark_brushes(hwnd);

            let hwnd_ctl = HWND(lparam.0 as *mut core::ffi::c_void);

            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00FFFFFF));

            let style = GetWindowLongPtrW(hwnd_ctl, GWL_STYLE) as u32;
            if (style & (BS_GROUPBOX as u32)) != 0 {
                let null_brush = GetStockObject(NULL_BRUSH);
                return LRESULT(null_brush.0 as isize);
            }

            SetBkColor(hdc, DARK_CONTROL_BG);

            if let Some(state) = get_state(hwnd) {
                let brush = state.dark_brush_control_bg;
                return LRESULT(brush.0 as isize);
            }

            return LRESULT(0);
        }

        SetTextColor(hdc, COLORREF(GetSysColor(COLOR_WINDOWTEXT)));
        SetBkMode(hdc, TRANSPARENT);

        let brush: HBRUSH = GetSysColorBrush(COLOR_WINDOW);
        LRESULT(brush.0 as isize)
    }
}

/// Handles `WM_CTLCOLORSTATIC` messages for static controls.
pub fn on_color_dialog(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(state) = get_state(hwnd)
        && state.current_theme_dark
    {
        ensure_dark_brushes(hwnd);

        let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
            SetBkColor(hdc, DARK_WINDOW_BG);
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
        }

        if let Some(state) = get_state(hwnd) {
            let brush = state.dark_brush_window_bg;
            return LRESULT(brush.0 as isize);
        }

        return on_ctlcolor(hwnd, wparam, lparam);
    }

    on_ctlcolor(hwnd, wparam, lparam)
}

/// Handles `WM_CTLCOLORSTATIC` messages for static controls.
/// Expected usage: called from a window procedure when processing `WM_CTLCOLORSTATIC`
/// messages.
pub fn on_color_static(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(state) = get_state(hwnd)
        && state.current_theme_dark
    {
        ensure_dark_brushes(hwnd);

        let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
            SetBkColor(hdc, DARK_CONTROL_BG);
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
        }

        if let Some(state) = get_state(hwnd) {
            let brush = state.dark_brush_control_bg;
            return LRESULT(brush.0 as isize);
        }

        return on_ctlcolor(hwnd, wparam, lparam);
    }

    on_ctlcolor(hwnd, wparam, lparam)
}

/// Handles `WM_CTLCOLOREDIT` messages for edit controls.
pub fn on_color_edit(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(state) = get_state(hwnd)
        && state.current_theme_dark
    {
        ensure_dark_brushes(hwnd);

        let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
            SetBkColor(hdc, DARK_EDIT_BG);
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
        }

        let brush = state.dark_brush_edit_bg;
        return LRESULT(brush.0 as isize);
    }

    on_ctlcolor(hwnd, wparam, lparam)
}

/// Handles `WM_ERASEBKGND` messages for window background erasing.
pub fn on_erase_background(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    let Some(state) = get_state(hwnd) else {
        return LRESULT(0);
    };

    let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
    let mut rect = RECT::default();

    unsafe {
        let _ = GetClientRect(hwnd, &mut rect);

        if state.current_theme_dark {
            ensure_dark_brushes(hwnd);

            if let Some(state) = get_state(hwnd) {
                let brush = state.dark_brush_window_bg;
                FillRect(hdc, &rect, brush);
                return LRESULT(1);
            }

            return LRESULT(0);
        }

        let brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
        FillRect(hdc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ::from(brush));

        LRESULT(1)
    }
}

/// Sets the window theme to dark or light mode based on `dark`.
/// Also forces a repaint of the window and its child controls to apply the theme changes.
pub fn set_window_theme(hwnd_main: HWND, dark: bool) {
    unsafe {
        let immersive = if dark { BOOL(1) } else { BOOL(0) };

        let _ = DwmSetWindowAttribute(
            hwnd_main,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &immersive as *const _ as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );

        let class = if dark {
            w!("DarkMode_Explorer")
        } else {
            w!("Explorer")
        };

        let _ = SetWindowTheme(hwnd_main, class, windows::core::PCWSTR::null());

        // Apply the same theme to child controls, without EnumChildWindows callback
        let mut child = GetWindow(hwnd_main, GW_CHILD).unwrap_or_default();

        while !child.0.is_null() {
            let _ = SetWindowTheme(child, class, windows::core::PCWSTR::null());
            child = GetWindow(child, GW_HWNDNEXT).unwrap_or_default();
        }

        with_state_mut_do(hwnd_main, |state| {
            state.current_theme_dark = dark;
        });

        let _ = InvalidateRect(Some(hwnd_main), None, true);
        let _ = UpdateWindow(hwnd_main);

        let flags = RDW_INVALIDATE | RDW_ALLCHILDREN;
        let _ = RedrawWindow(Some(hwnd_main), None, None, flags);
    }
}
