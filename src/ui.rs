//! Construction of the graphical user interface.
//!
//! This module contains routines for laying out the settings window
//! and populating it with controls.  Layout values and control
//! creation are kept here to keep the message loop free of UI
//! details.

use crate::app::{AppState, ID_APPLY, ID_AUTOSTART, ID_CANCEL, ID_DELAY_MS, ID_EXIT, ID_TRAY};
use crate::helpers::ws_i32;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_AUTOCHECKBOX, BS_GROUPBOX, CreateWindowExW, ES_NUMBER, ES_READONLY, GetClientRect, HMENU,
    SetWindowTextW, WINDOW_EX_STYLE, WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::PCWSTR;
use windows::core::w;

/// Internal helper which draws a label and a read‑only edit control on
/// the same horizontal row.  A mutable reference to an `HWND` is
/// passed so that the created edit control handle can be stored in
/// the caller's state.
fn hotkey_row(
    parent: HWND,
    x: i32,
    y: i32,
    w_label: i32,
    w_edit: i32,
    label: PCWSTR,
    value: PCWSTR,
    out_edit: &mut HWND,
) -> windows::core::Result<()> {
    unsafe {
        // Label describing the hotkey action
        let _lbl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            label,
            WS_CHILD | WS_VISIBLE,
            x,
            y + 3,
            w_label,
            18,
            Some(parent),
            None,
            None,
            None,
        )?;

        // Read‑only edit control showing the current shortcut
        *out_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            value,
            ws_i32(WS_CHILD | WS_VISIBLE, ES_READONLY),
            x + w_label + 8,
            y,
            w_edit,
            22,
            Some(parent),
            None,
            None,
            None,
        )?;

        Ok(())
    }
}

/// Create all of the controls for the settings window.
///
/// The main window owns all child controls; therefore the `hwnd`
/// parameter is the parent for every control.  Coordinates are
/// specified relative to the client area of the parent.  Upon
/// completion the `state` structure contains the handles of all
/// created controls.
pub fn create_controls(hwnd: HWND, state: &mut AppState) -> windows::core::Result<()> {
    unsafe {
        // Determine the size of the client area for reference.  The
        // values are not currently used but retained for potential
        // dynamic layout adjustments in the future.
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let _w = rc.right - rc.left;
        let _h = rc.bottom - rc.top;

        // Layout constants
        let margin = 12;
        let group_h = 170;
        let group_w_left = 240;
        let gap = 12;
        let group_w_right = 260;

        let left_x = margin;
        let top_y = margin;
        let right_x = left_x + group_w_left + gap;

        // Settings group box
        let _grp_settings = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Settings"),
            ws_i32(WS_CHILD | WS_VISIBLE, BS_GROUPBOX),
            left_x,
            top_y,
            group_w_left,
            group_h,
            Some(hwnd),
            None,
            None,
            None,
        )?;

        // Autostart checkbox
        state.chk_autostart = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Start on startup"),
            ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            left_x + 12,
            top_y + 28,
            group_w_left - 24,
            20,
            Some(hwnd),
            Some(HMENU(ID_AUTOSTART as *mut _)),
            None,
            None,
        )?;

        // Tray icon checkbox
        state.chk_tray = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Show tray icon"),
            ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            left_x + 12,
            top_y + 52,
            group_w_left - 24,
            20,
            Some(hwnd),
            Some(HMENU(ID_TRAY as *mut _)),
            None,
            None,
        )?;

        // Delay label
        let _lbl_delay = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("Delay before switching:"),
            WS_CHILD | WS_VISIBLE,
            left_x + 12,
            top_y + 82,
            group_w_left - 24,
            18,
            Some(hwnd),
            None,
            None,
            None,
        )?;

        // Delay input
        state.edit_delay_ms = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            w!("100"),
            ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, ES_NUMBER),
            left_x + 12,
            top_y + 104,
            60,
            22,
            Some(hwnd),
            Some(HMENU(ID_DELAY_MS as *mut _)),
            None,
            None,
        )?;

        // Milliseconds label
        let _lbl_ms = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("ms"),
            WS_CHILD | WS_VISIBLE,
            left_x + 78,
            top_y + 107,
            24,
            18,
            Some(hwnd),
            None,
            None,
            None,
        )?;

        // Hotkeys group box
        let _grp_hotkeys = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Hotkeys"),
            ws_i32(WS_CHILD | WS_VISIBLE, BS_GROUPBOX),
            right_x,
            top_y,
            group_w_right,
            group_h,
            Some(hwnd),
            None,
            None,
            None,
        )?;

        // Position and sizes for hotkey rows
        let hx = right_x + 12;
        let mut hy = top_y + 28;
        let w_label = 130;
        let w_edit = group_w_right - 12 - 12 - w_label - 8;

        // Convert last word hotkey
        hotkey_row(
            hwnd,
            hx,
            hy,
            w_label,
            w_edit,
            w!("Convert last word:"),
            w!("DoubleShift"),
            &mut state.edit_hotkey_last_word,
        )?;
        hy += 28;

        // Pause hotkey
        hotkey_row(
            hwnd,
            hx,
            hy,
            w_label,
            w_edit,
            w!("Pause:"),
            w!(""),
            &mut state.edit_hotkey_pause,
        )?;
        hy += 28;

        // Convert selection hotkey
        hotkey_row(
            hwnd,
            hx,
            hy,
            w_label,
            w_edit,
            w!("Convert selection:"),
            w!("Ctrl + Cancel"),
            &mut state.edit_hotkey_selection,
        )?;
        hy += 28;

        // Switch layout hotkey
        hotkey_row(
            hwnd,
            hx,
            hy,
            w_label,
            w_edit,
            w!("Switch keyboard layout:"),
            w!(""),
            &mut state.edit_hotkey_switch_layout,
        )?;

        // Common button layout
        let btn_y = top_y + group_h + 10;
        let btn_h = 28;
        let btn_style = WS_CHILD | WS_VISIBLE | WS_TABSTOP;

        // Exit button
        state.btn_exit = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Exit"),
            btn_style,
            left_x + 12,
            btn_y,
            110,
            btn_h,
            Some(hwnd),
            Some(HMENU(ID_EXIT as *mut _)),
            None,
            None,
        )?;

        // Apply button
        state.btn_apply = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Apply"),
            btn_style,
            right_x + 40,
            btn_y,
            90,
            btn_h,
            Some(hwnd),
            Some(HMENU(ID_APPLY as *mut _)),
            None,
            None,
        )?;

        // Cancel button
        state.btn_cancel = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Cancel"),
            btn_style,
            right_x + 140,
            btn_y,
            90,
            btn_h,
            Some(hwnd),
            Some(HMENU(ID_CANCEL as *mut _)),
            None,
            None,
        )?;

        // Optionally set the default button text again – the returned
        // handle already contains the caption, but the original code
        // did this as a safety measure.
        let _ = SetWindowTextW(state.btn_apply, w!("Apply"));

        Ok(())
    }
}
