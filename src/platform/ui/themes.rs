//! Theme management for Windows UI
//!
//! This module provides theme-aware painting functions for Windows controls
//! and handles dark/light theme switching.
//!
//! Goals:
//! - strong typing for theme selection
//! - centralized theme palette
//! - minimal Win32 glue at the edges
//! - pure functions for all "decide colors" logic
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::{
            Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
            Gdi::{
                BeginPaint, COLOR_WINDOW, COLOR_WINDOWTEXT, CreatePen, CreateSolidBrush,
                DEFAULT_GUI_FONT, DT_CALCRECT, DT_CENTER, DT_LEFT, DT_SINGLELINE, DT_VCENTER,
                DeleteObject, DrawFocusRect, DrawTextW, EndPaint, FillRect, FrameRect,
                GetStockObject, GetSysColor, GetSysColorBrush, HBRUSH, HDC, HGDIOBJ,
                InvalidateRect, LineTo, MoveToEx, PAINTSTRUCT, PS_SOLID, RDW_ALLCHILDREN,
                RDW_ERASE, RDW_INVALIDATE, RedrawWindow, SelectObject, SetBkColor, SetBkMode,
                SetTextColor, TRANSPARENT, UpdateWindow,
            },
        },
        UI::{
            Controls::{
                DRAWITEMSTRUCT, ODS_DEFAULT, ODS_DISABLED, ODS_FOCUS, ODS_SELECTED, ODT_BUTTON,
                SetWindowTheme,
            },
            WindowsAndMessaging::{GetClientRect, GetParent, GetWindow, GetWindowLongPtrW, *},
        },
    },
    core::{BOOL, w},
};

use crate::platform::{
    ui::geom::{Layout, RectI},
    win::state::{get_state, with_state_mut_do},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn is_dark(self) -> bool {
        matches!(self, Theme::Dark)
    }

    pub fn colors(self) -> &'static ThemeColors {
        match self {
            Theme::Light => &THEME_LIGHT,
            Theme::Dark => &THEME_DARK,
        }
    }

    pub fn button_palette(self) -> &'static ButtonPalette {
        match self {
            Theme::Light => &BUTTON_LIGHT,
            Theme::Dark => &BUTTON_DARK,
        }
    }
}

const ACCENT_CORAL: COLORREF = COLORREF(0x000048F0);

#[derive(Copy, Clone, Debug)]
pub struct ThemeColors {
    pub window_bg: COLORREF,
    pub control_bg: COLORREF,
    pub edit_bg: COLORREF,
    pub text: COLORREF,
}

const THEME_DARK: ThemeColors = ThemeColors {
    window_bg: COLORREF(0x002D2D30),
    control_bg: COLORREF(0x002D2D30),
    edit_bg: COLORREF(0x002D2D30),
    text: COLORREF(0x00FFFFFF),
};

const THEME_LIGHT: ThemeColors = ThemeColors {
    window_bg: COLORREF(0x00FFFFFF),
    control_bg: COLORREF(0x00FFFFFF),
    edit_bg: COLORREF(0x00FFFFFF),
    text: COLORREF(0x00000000),
};

#[derive(Copy, Clone, Debug)]
pub struct ButtonPalette {
    pub bg_normal: COLORREF,
    pub bg_pressed: COLORREF,
    pub bg_focused: COLORREF,
    pub bg_disabled: COLORREF,

    pub text_normal: COLORREF,
    pub text_disabled: COLORREF,

    pub border_normal: COLORREF,
    pub border_pressed: COLORREF,
    pub border_disabled: COLORREF,
}

const BUTTON_DARK: ButtonPalette = ButtonPalette {
    bg_normal: COLORREF(0x00262626),
    bg_pressed: COLORREF(0x003C3C3C),
    bg_focused: COLORREF(0x00323232),
    bg_disabled: COLORREF(0x00333333),

    text_normal: COLORREF(0x00FFFFFF),
    text_disabled: COLORREF(0x00888888),

    border_normal: COLORREF(0x00404040),
    border_pressed: COLORREF(0x00505050),
    border_disabled: COLORREF(0x00404040),
};

const BUTTON_LIGHT: ButtonPalette = ButtonPalette {
    bg_normal: COLORREF(0x00F0F0F0),
    bg_pressed: COLORREF(0x00C0C0C0),
    bg_focused: COLORREF(0x00E0E0E0),
    bg_disabled: COLORREF(0x00C0C0C0),

    text_normal: COLORREF(0x00000000),
    text_disabled: COLORREF(0x00808080),

    border_normal: COLORREF(0x00808080),
    border_pressed: COLORREF(0x00808080),
    border_disabled: COLORREF(0x00808080),
};

fn resolve_theme_owner_hwnd(mut hwnd: HWND) -> HWND {
    for _ in 0..64 {
        if get_state(hwnd).is_some() {
            return hwnd;
        }

        let parent = unsafe { GetParent(hwnd).unwrap_or_default() };
        if parent.0.is_null() || parent == hwnd {
            return hwnd;
        }
        hwnd = parent;
    }

    hwnd
}

fn theme_for_hwnd(hwnd: HWND) -> (Theme, HWND) {
    let owner = resolve_theme_owner_hwnd(hwnd);
    let dark = get_state(owner)
        .map(|s| s.current_theme_dark)
        .unwrap_or(false);

    let theme = if dark { Theme::Dark } else { Theme::Light };
    (theme, owner)
}

fn ensure_dark_brushes(owner: HWND, colors: &ThemeColors) {
    with_state_mut_do(owner, |state| unsafe {
        if state.dark_brush_window_bg.0.is_null() {
            state.dark_brush_window_bg = CreateSolidBrush(colors.window_bg);
        }
        if state.dark_brush_control_bg.0.is_null() {
            state.dark_brush_control_bg = CreateSolidBrush(colors.control_bg);
        }
        if state.dark_brush_edit_bg.0.is_null() {
            state.dark_brush_edit_bg = CreateSolidBrush(colors.edit_bg);
        }
    });
}

struct OwnedBrush(HBRUSH);

impl OwnedBrush {
    fn new(color: COLORREF) -> Self {
        unsafe { Self(CreateSolidBrush(color)) }
    }

    fn as_hbrush(&self) -> HBRUSH {
        self.0
    }
}

impl Drop for OwnedBrush {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ::from(self.0));
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct ButtonState {
    pressed: bool,
    focused: bool,
    defaulted: bool,
    disabled: bool,
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

fn choose_button_colors(
    state: ButtonState,
    palette: &ButtonPalette,
) -> (COLORREF, COLORREF, COLORREF) {
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
    let brush = OwnedBrush::new(bg);
    unsafe {
        FillRect(hdc, rect, brush.as_hbrush());
    }
}

fn paint_button_border(hdc: HDC, rect: &RECT, border: COLORREF) {
    let brush = OwnedBrush::new(border);
    unsafe {
        FrameRect(hdc, rect, brush.as_hbrush());
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
    state: ButtonState,
    palette: &ButtonPalette,
) {
    let (mut text_buf, text_len) = read_button_text(hwnd_btn);
    let (bg, text, border) = choose_button_colors(state, palette);

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

        let (theme, _owner) = theme_for_hwnd(parent_hwnd);
        let palette = theme.button_palette();
        let state = read_button_state(draw_item);

        paint_button(
            draw_item.hDC,
            &draw_item.rcItem,
            draw_item.hwndItem,
            state,
            palette,
        );

        LRESULT(1)
    }
}

#[derive(Copy, Clone, Debug)]
enum ControlRole {
    Static,
    Dialog,
    Edit,
}

fn is_groupbox(hwnd_ctl: HWND) -> bool {
    const BS_TYPEMASK_U32: u32 = 0x0000000F;

    unsafe {
        let style = GetWindowLongPtrW(hwnd_ctl, GWL_STYLE) as u32;
        (style & BS_TYPEMASK_U32) == (BS_GROUPBOX as u32)
    }
}

fn paint_group_frame(
    hdc: HDC,
    r: RectI,
    title: &str,
    text_color: COLORREF,
    border_color: COLORREF,
    background_color: COLORREF,
) {
    unsafe {
        // Шрифт как у системы
        let old_font = SelectObject(hdc, GetStockObject(DEFAULT_GUI_FONT));

        // Меряем текст ТОЛЬКО ширину и высоту, без VCENTER
        let mut title_buf: Vec<u16> = title.encode_utf16().collect();

        let mut measure = RECT::default();
        let _ = DrawTextW(
            hdc,
            &mut title_buf,
            &mut measure,
            DT_LEFT | DT_SINGLELINE | DT_CALCRECT,
        );

        let text_w = measure.right - measure.left;
        let text_h = measure.bottom - measure.top;

        // Верхняя линия рамки через середину высоты текста
        let line_y = r.y + (text_h / 2);

        // Координаты рамки
        let left = r.x;
        let right = r.x + r.w;
        let top = line_y;
        let bottom = r.y + r.h;

        // Рисуем рамку как линии
        let pen = CreatePen(PS_SOLID, 1, border_color);
        let old_pen = SelectObject(hdc, pen.into());

        // Left
        let _ = MoveToEx(hdc, left, top, None);
        let _ = LineTo(hdc, left, bottom);

        // Bottom
        let _ = MoveToEx(hdc, left, bottom, None);
        let _ = LineTo(hdc, right, bottom);

        // Right
        let _ = MoveToEx(hdc, right, top, None);
        let _ = LineTo(hdc, right, bottom);

        // Top: два сегмента, дырка под заголовок
        let pad_left = 12;
        let pad_right = 8;

        let text_left = left + pad_left;
        let text_right = text_left + text_w + pad_right;

        // Top left segment
        let _ = MoveToEx(hdc, left, top, None);
        let _ = LineTo(hdc, text_left - 4, top);

        // Top right segment
        let _ = MoveToEx(hdc, text_right, top, None);
        let _ = LineTo(hdc, right, top);

        // Подложка под текстом, чтобы стереть линию
        let bg_brush = OwnedBrush::new(background_color);

        // Даем запас по высоте, чтобы не клипало сверху и снизу
        let pad_y_top = 2;
        let pad_y_bottom = 2;

        let cap_bg = RECT {
            left: text_left - 2,
            top: r.y,
            right: text_right + 1,
            bottom: r.y + text_h + pad_y_top + pad_y_bottom,
        };

        FillRect(hdc, &cap_bg, bg_brush.as_hbrush());

        // Текст рисуем без VCENTER, в более высоком прямоугольнике
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, text_color);

        let mut cap_text = RECT {
            left: text_left,
            top: r.y + pad_y_top,
            right: text_right,
            bottom: r.y + pad_y_top + text_h + pad_y_bottom,
        };

        DrawTextW(hdc, &mut title_buf, &mut cap_text, DT_LEFT | DT_SINGLELINE);

        // Cleanup
        let _ = SelectObject(hdc, old_pen);
        let _ = DeleteObject(HGDIOBJ::from(pen));
        let _ = SelectObject(hdc, old_font);
    }
}

pub fn on_paint(hwnd: HWND, _wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);

        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let client_w = rc.right - rc.left;

        let l = Layout::new(client_w);

        let (theme, _owner) = theme_for_hwnd(hwnd);
        let colors = theme.colors();

        let border = ACCENT_CORAL;

        let settings_rect = RectI::new(l.left_x(), l.top_y(), l.group_w_left(), l.group_h());
        let hotkeys_rect = RectI::new(l.right_x(), l.top_y(), l.group_w_right(), l.group_h());

        paint_group_frame(
            hdc,
            settings_rect,
            "Settings",
            colors.text,
            border,
            colors.window_bg,
        );

        paint_group_frame(
            hdc,
            hotkeys_rect,
            "Hotkeys",
            colors.text,
            border,
            colors.window_bg,
        );

        let _ = EndPaint(hwnd, &ps);
        LRESULT(0)
    }
}

fn apply_dark_ctlcolor(owner: HWND, hdc: HDC, hwnd_ctl: HWND, role: ControlRole) -> HBRUSH {
    let colors = Theme::Dark.colors();
    ensure_dark_brushes(owner, colors);

    unsafe {
        SetTextColor(hdc, colors.text);
    }

    match role {
        ControlRole::Edit => unsafe {
            SetBkMode(hdc, windows::Win32::Graphics::Gdi::OPAQUE);
        },
        _ => unsafe {
            SetBkMode(hdc, TRANSPARENT);
        },
    }

    if is_groupbox(hwnd_ctl) {
        let Some(state) = get_state(owner) else {
            return HBRUSH::default();
        };

        unsafe {
            SetBkColor(hdc, colors.control_bg);
        }

        return state.dark_brush_control_bg;
    }

    let bg = match role {
        ControlRole::Dialog => colors.window_bg,
        ControlRole::Edit => colors.edit_bg,
        ControlRole::Static => colors.control_bg,
    };

    unsafe {
        SetBkColor(hdc, bg);
    }

    let Some(state) = get_state(owner) else {
        return HBRUSH::default();
    };

    match role {
        ControlRole::Dialog => state.dark_brush_window_bg,
        ControlRole::Edit => state.dark_brush_edit_bg,
        ControlRole::Static => state.dark_brush_control_bg,
    }
}

fn apply_light_ctlcolor(hdc: HDC) -> HBRUSH {
    unsafe {
        SetTextColor(hdc, COLORREF(GetSysColor(COLOR_WINDOWTEXT)));
        SetBkMode(hdc, TRANSPARENT);
        GetSysColorBrush(COLOR_WINDOW)
    }
}

fn ctlcolor(hwnd_any: HWND, wparam: WPARAM, lparam: LPARAM, role: ControlRole) -> LRESULT {
    let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
    let hwnd_ctl = HWND(lparam.0 as *mut core::ffi::c_void);

    let (theme, owner) = theme_for_hwnd(hwnd_any);

    let brush = match theme {
        Theme::Dark => apply_dark_ctlcolor(owner, hdc, hwnd_ctl, role),
        Theme::Light => apply_light_ctlcolor(hdc),
    };

    LRESULT(brush.0 as isize)
}

/// Handles `WM_CTLCOLOR*` style messages by configuring the device context and returning a brush.
pub fn on_ctlcolor(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    ctlcolor(hwnd, wparam, lparam, ControlRole::Static)
}

/// Handles `WM_CTLCOLORSTATIC` messages for static controls.
pub fn on_color_dialog(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    ctlcolor(hwnd, wparam, lparam, ControlRole::Dialog)
}

/// Handles `WM_CTLCOLORSTATIC` messages for static controls.
pub fn on_color_static(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    ctlcolor(hwnd, wparam, lparam, ControlRole::Static)
}

/// Handles `WM_CTLCOLOREDIT` messages for edit controls.
pub fn on_color_edit(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    ctlcolor(hwnd, wparam, lparam, ControlRole::Edit)
}

/// Handles `WM_ERASEBKGND` messages for window background erasing.
pub fn on_erase_background(hwnd: HWND, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    let (theme, owner) = theme_for_hwnd(hwnd);

    let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
    let mut rect = RECT::default();

    unsafe {
        let _ = GetClientRect(hwnd, &mut rect);
    }

    match theme {
        Theme::Dark => {
            let colors = Theme::Dark.colors();
            ensure_dark_brushes(owner, colors);

            let Some(state) = get_state(owner) else {
                return LRESULT(0);
            };

            unsafe {
                FillRect(hdc, &rect, state.dark_brush_window_bg);
            }
            LRESULT(1)
        }
        Theme::Light => unsafe {
            let brush = GetSysColorBrush(COLOR_WINDOW);
            FillRect(hdc, &rect, brush);
            LRESULT(1)
        },
    }
}

/// Sets the window theme to dark or light mode based on `dark`.
/// Also forces a repaint of the window and its child controls to apply the theme changes.
pub fn set_window_theme(hwnd_main: HWND, dark: bool) {
    unsafe {
        let theme = if dark { Theme::Dark } else { Theme::Light };

        // IMPORTANT: update state before anything that might trigger painting
        with_state_mut_do(hwnd_main, |state| {
            state.current_theme_dark = theme.is_dark();
        });

        let immersive = if theme.is_dark() { BOOL(1) } else { BOOL(0) };
        let _ = DwmSetWindowAttribute(
            hwnd_main,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &immersive as *const _ as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );

        let class = if theme.is_dark() {
            w!("DarkMode_Explorer")
        } else {
            w!("Explorer")
        };

        let _ = SetWindowTheme(hwnd_main, class, windows::core::PCWSTR::null());

        let mut child = GetWindow(hwnd_main, GW_CHILD).unwrap_or_default();
        while !child.0.is_null() {
            let _ = SetWindowTheme(child, class, windows::core::PCWSTR::null());
            child = GetWindow(child, GW_HWNDNEXT).unwrap_or_default();
        }

        let _ = InvalidateRect(Some(hwnd_main), None, true);
        let _ = UpdateWindow(hwnd_main);

        let flags = RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN;
        let _ = RedrawWindow(Some(hwnd_main), None, None, flags);
    }
}
