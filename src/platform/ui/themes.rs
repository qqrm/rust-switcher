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
                GetSysColor, GetSysColorBrush, HBRUSH, HDC, HGDIOBJ, InvalidateRect,
                RDW_ALLCHILDREN, RDW_INVALIDATE, REDRAW_WINDOW_FLAGS, RedrawWindow, SetBkColor,
                SetBkMode, SetTextColor, TRANSPARENT, UpdateWindow,
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

/// Handles `WM_DRAWITEM` messages for owner-drawn buttons.
/// Expected usage: called from a window procedure when processing `WM_DRAWITEM`
/// What it does:
/// - checks if the control being drawn is a button
/// - retrieves the application state to determine the current theme
/// - paints the button according to the current theme (dark or light)
/// - handles button states like pressed, focused, default, and disabled
pub fn on_draw_item(_hwnd: HWND, _wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let draw_item = &*(lparam.0 as *const DRAWITEMSTRUCT);

        // Check if it's a button
        if draw_item.CtlType == ODT_BUTTON {
            // Get the main window state (draw_item.hwndItem is the button itself)
            // We need to get the parent window to access state
            let parent_hwnd = match GetParent(draw_item.hwndItem) {
                Ok(hwnd) => hwnd,
                Err(_) => return LRESULT(0), // Can't get parent, skip drawing
            };

            match get_state(parent_hwnd) {
                Some(state) if state.current_theme_dark => {
                    // DARK THEME BUTTON PAINTING
                    let hdc = draw_item.hDC;
                    let rect = draw_item.rcItem;

                    // Get button text
                    let mut text_buffer = [0u16; 256];
                    let text_len = GetWindowTextW(draw_item.hwndItem, &mut text_buffer) as usize;
                    //let text = &text_buffer[..text_len as usize];

                    // Check button state
                    let is_pressed = (draw_item.itemState.0 & ODS_SELECTED.0) != 0;
                    let is_focused = (draw_item.itemState.0 & ODS_FOCUS.0) != 0;
                    let is_default = (draw_item.itemState.0 & ODS_DEFAULT.0) != 0;

                    // Dark theme colors
                    let background_color = if is_pressed {
                        COLORREF(0x00404040) // Dark gray when pressed
                    } else if is_focused || is_default {
                        COLORREF(0x00303030) // Slightly lighter gray when focused
                    } else {
                        COLORREF(0x00000000) // Black for normal state
                    };

                    let text_color = COLORREF(0x00FFFFFF); // White text

                    // Paint button background
                    let brush = CreateSolidBrush(background_color);
                    FillRect(hdc, &rect, brush);
                    let _ = DeleteObject(HGDIOBJ::from(brush));

                    // Draw focus rectangle if needed
                    if is_focused && !is_pressed {
                        let focus_rect = RECT {
                            left: rect.left + 3,
                            top: rect.top + 3,
                            right: rect.right - 3,
                            bottom: rect.bottom - 3,
                        };
                        let _ = DrawFocusRect(hdc, &focus_rect);
                    }

                    // Draw button border
                    let border_brush = if is_pressed {
                        CreateSolidBrush(COLORREF(0x00606060)) // Light gray border when pressed
                    } else {
                        CreateSolidBrush(COLORREF(0x00404040)) // Gray border
                    };

                    FrameRect(hdc, &rect, border_brush);
                    let _ = DeleteObject(HGDIOBJ::from(border_brush));

                    // Draw button text
                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, text_color);

                    // Adjust text position if pressed (gives "pressed" effect)
                    let mut text_rect = rect;
                    if is_pressed {
                        text_rect.left += 2;
                        text_rect.top += 2;
                        text_rect.right += 2;
                        text_rect.bottom += 2;
                    }

                    DrawTextW(
                        hdc,
                        &mut text_buffer[..text_len], // Slice with actual text
                        &mut text_rect,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );

                    return LRESULT(1); // We handled the drawing
                }
                Some(_) => {
                    // LIGHT THEME - Draw light theme button
                    let hdc = draw_item.hDC;
                    let rect = draw_item.rcItem;

                    // Get button text
                    let mut text_buffer = [0u16; 256];
                    let text_len = GetWindowTextW(draw_item.hwndItem, &mut text_buffer) as usize;

                    // Check button state
                    let is_pressed = (draw_item.itemState.0 & ODS_SELECTED.0) != 0;
                    let is_focused = (draw_item.itemState.0 & ODS_FOCUS.0) != 0;
                    let is_default = (draw_item.itemState.0 & ODS_DEFAULT.0) != 0;
                    let is_disabled = (draw_item.itemState.0 & ODS_DISABLED.0) != 0;

                    if is_disabled {
                        // Draw disabled button appearance (light theme)
                        let brush = CreateSolidBrush(COLORREF(0x00C0C0C0)); // Light gray
                        FillRect(hdc, &rect, brush);
                        let _ = DeleteObject(HGDIOBJ::from(brush));

                        let border_brush = CreateSolidBrush(COLORREF(0x00808080));
                        FrameRect(hdc, &rect, border_brush);
                        let _ = DeleteObject(HGDIOBJ::from(border_brush));

                        SetBkMode(hdc, TRANSPARENT);
                        SetTextColor(hdc, COLORREF(0x00808080)); // Gray text
                    } else {
                        // Normal light theme button
                        let background_color = if is_pressed {
                            COLORREF(0x00C0C0C0) // Light gray when pressed
                        } else if is_focused || is_default {
                            COLORREF(0x00E0E0E0) // Lighter gray when focused
                        } else {
                            COLORREF(0x00F0F0F0) // Very light gray
                        };

                        // Paint button background
                        let brush = CreateSolidBrush(background_color);
                        FillRect(hdc, &rect, brush);
                        let _ = DeleteObject(HGDIOBJ::from(brush));

                        // Draw button border
                        let border_brush = CreateSolidBrush(COLORREF(0x00808080));
                        FrameRect(hdc, &rect, border_brush);
                        let _ = DeleteObject(HGDIOBJ::from(border_brush));

                        SetBkMode(hdc, TRANSPARENT);
                        SetTextColor(hdc, COLORREF(0x00000000)); // Black text
                    }

                    // Draw text
                    let mut text_rect = rect;
                    if is_pressed && !is_disabled {
                        text_rect.left += 2;
                        text_rect.top += 2;
                        text_rect.right += 2;
                        text_rect.bottom += 2;
                    }

                    DrawTextW(
                        hdc,
                        &mut text_buffer[..text_len],
                        &mut text_rect,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );

                    return LRESULT(1); // We handled the drawing
                }
                None => {
                    // Handle button when no state available
                    let hdc = draw_item.hDC;
                    let rect = draw_item.rcItem;

                    // Get button text
                    let mut text_buffer = [0u16; 256];
                    let text_len = GetWindowTextW(draw_item.hwndItem, &mut text_buffer) as usize;

                    // Check button state
                    let is_pressed = (draw_item.itemState.0 & ODS_SELECTED.0) != 0;
                    let is_focused = (draw_item.itemState.0 & ODS_FOCUS.0) != 0;
                    let is_default = (draw_item.itemState.0 & ODS_DEFAULT.0) != 0;
                    let is_disabled = (draw_item.itemState.0 & ODS_DISABLED.0) != 0;

                    if is_disabled {
                        // Draw disabled button appearance
                        let brush = CreateSolidBrush(COLORREF(0x00C0C0C0)); // Light gray background
                        FillRect(hdc, &rect, brush);
                        let _ = DeleteObject(HGDIOBJ::from(brush));

                        // Draw button border
                        let border_brush = CreateSolidBrush(COLORREF(0x00808080)); // Gray border
                        FrameRect(hdc, &rect, border_brush);
                        let _ = DeleteObject(HGDIOBJ::from(border_brush));

                        // Gray text for disabled
                        SetBkMode(hdc, TRANSPARENT);
                        SetTextColor(hdc, COLORREF(0x00808080));
                    } else {
                        // Normal light theme button
                        let background_color = if is_pressed {
                            COLORREF(0x00C0C0C0) // Light gray when pressed
                        } else if is_focused || is_default {
                            COLORREF(0x00E0E0E0) // Lighter gray when focused
                        } else {
                            COLORREF(0x00F0F0F0) // Very light gray for normal
                        };

                        let text_color = COLORREF(0x00000000); // Black text

                        // Paint button background
                        let brush = CreateSolidBrush(background_color);
                        FillRect(hdc, &rect, brush);
                        let _ = DeleteObject(HGDIOBJ::from(brush));

                        // Draw button border
                        let border_brush = CreateSolidBrush(COLORREF(0x00808080)); // Gray border
                        FrameRect(hdc, &rect, border_brush);
                        let _ = DeleteObject(HGDIOBJ::from(border_brush));

                        // Draw button text
                        SetBkMode(hdc, TRANSPARENT);
                        SetTextColor(hdc, text_color);
                    }

                    // Draw text (common for both disabled and enabled)
                    let mut text_rect = rect;
                    if is_pressed && !is_disabled {
                        text_rect.left += 2;
                        text_rect.top += 2;
                        text_rect.right += 2;
                        text_rect.bottom += 2;
                    }

                    DrawTextW(
                        hdc,
                        &mut text_buffer[..text_len],
                        &mut text_rect,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );

                    return LRESULT(1); // We handled the drawing
                }
            }
        }

        LRESULT(0) // Not a button or we didn't handle it
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
pub fn on_ctlcolor(wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    unsafe {
        let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
        SetTextColor(hdc, COLORREF(GetSysColor(COLOR_WINDOWTEXT)));
        SetBkMode(hdc, TRANSPARENT);

        let brush: HBRUSH = GetSysColorBrush(COLOR_WINDOW);
        LRESULT(brush.0 as isize)
    }
}

/// Handles `WM_CTLCOLORSTATIC` messages for static controls.
pub fn on_color_dialog(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    if let Some(state) = get_state(hwnd)
        && state.current_theme_dark
    {
        let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
        unsafe {
            SetBkColor(hdc, COLORREF(0x002D2D30));
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
            return LRESULT(CreateSolidBrush(COLORREF(0x002D2D30)).0 as isize);
        }
    }
    on_ctlcolor(wparam, _lparam)
}

/// Handles `WM_CTLCOLORSTATIC` messages for static controls.
/// Expected usage: called from a window procedure when processing `WM_CTLCOLORSTATIC`
/// messages.
pub fn on_color_static(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    if let Some(state) = get_state(hwnd)
        && state.current_theme_dark
    {
        let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
        unsafe {
            SetBkColor(hdc, COLORREF(0x002D2D30));
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
            return LRESULT(CreateSolidBrush(COLORREF(0x002D2D30)).0 as isize);
        }
    }
    on_ctlcolor(wparam, _lparam)
}

/// Handles `WM_CTLCOLOREDIT` messages for edit controls.
pub fn on_color_edit(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    if let Some(state) = get_state(hwnd)
        && state.current_theme_dark
    {
        let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
        unsafe {
            SetBkColor(hdc, COLORREF(0x001E1E1E)); // Dark gray for dark theme
            SetTextColor(hdc, COLORREF(0x00FFFFFF)); // White text for dark theme
            return LRESULT(CreateSolidBrush(COLORREF(0x002D2D30)).0 as isize);
        }
    }
    on_ctlcolor(wparam, _lparam)
}

/// Handles `WM_ERASEBKGND` messages for window background erasing.
pub fn on_erase_background(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    if let Some(state) = get_state(hwnd)
        && state.current_theme_dark
    {
        // Paint main window background
        let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
        let mut rect = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rect);
            let brush = CreateSolidBrush(COLORREF(0x002D2D30));
            FillRect(hdc, &rect, brush);
            let _ = DeleteObject(HGDIOBJ::from(brush));
        }
        LRESULT(1)
    } else {
        // Explicit light theme background (white)
        let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
        let mut rect = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rect);
            let brush = CreateSolidBrush(COLORREF(0x00FFFFFF)); // White
            // Or use system color: let brush = GetSysColorBrush(COLOR_WINDOW as i32);
            FillRect(hdc, &rect, brush);
            let _ = DeleteObject(HGDIOBJ::from(brush));
        }
        LRESULT(1)
    }
}

/// Sets the window theme to dark or light mode based on `current_theme_dark`.
/// Also forces a repaint of the window and its child controls to apply the theme changes.
pub fn set_window_theme(hwnd_main: HWND, current_theme_dark: bool) {
    unsafe {
        if !current_theme_dark {
            let mut dark_mode: BOOL = BOOL(1);

            let _ = DwmSetWindowAttribute(
                hwnd_main,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &mut dark_mode as *mut _ as *const _,
                std::mem::size_of::<BOOL>() as u32,
            );

            let _ = SetWindowTheme(
                hwnd_main,
                w!("DarkMode_Explorer"),
                windows::core::PCWSTR::null(),
            );
            with_state_mut_do(hwnd_main, |state| {
                state.current_theme_dark = true;
            });
        } else {
            // Revert to light mode
            let mut light_mode: BOOL = BOOL(0);
            let _ = DwmSetWindowAttribute(
                hwnd_main,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &mut light_mode as *mut _ as *const _,
                std::mem::size_of::<BOOL>() as u32,
            );
            let _ = SetWindowTheme(hwnd_main, w!("Explorer"), windows::core::PCWSTR::null());
            with_state_mut_do(hwnd_main, |state| {
                state.current_theme_dark = false;
            });
        }

        // Force window repaint to apply the theme changes
        let _ = InvalidateRect(Some(hwnd_main), None, true);
        let _ = UpdateWindow(hwnd_main);

        // Also redraw child controls (combine the typed flag values)
        let flags = REDRAW_WINDOW_FLAGS(RDW_INVALIDATE.0 | RDW_ALLCHILDREN.0);
        let _ = RedrawWindow(Some(hwnd_main), None, None, flags);
    }
}
