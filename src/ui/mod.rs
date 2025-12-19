mod geom;
use self::geom::*;

pub(crate) mod error_notifier;

use crate::app::{AppState, ControlId};
use crate::helpers::ws_i32;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_AUTOCHECKBOX, BS_GROUPBOX, CreateWindowExW, ES_NUMBER, ES_READONLY, GetClientRect,
    SetWindowTextW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP,
    WS_VISIBLE,
};
use windows::core::PCWSTR;
use windows::core::w;

#[derive(Clone, Copy)]
struct ControlSpec {
    ex_style: WINDOW_EX_STYLE,
    class: PCWSTR,
    text: PCWSTR,
    style: WINDOW_STYLE,
    rect: RectI,
    menu: Option<windows::Win32::UI::WindowsAndMessaging::HMENU>,
}

fn create(parent: HWND, s: ControlSpec) -> windows::core::Result<HWND> {
    unsafe {
        CreateWindowExW(
            s.ex_style,
            s.class,
            s.text,
            s.style,
            s.rect.x,
            s.rect.y,
            s.rect.w,
            s.rect.h,
            Some(parent),
            s.menu,
            None,
            None,
        )
    }
}

struct HotkeyRowSpec {
    label: ControlSpec,
    edit: ControlSpec,
}

fn hotkey_row_spec(
    x: i32,
    y: i32,
    w_label: i32,
    w_edit: i32,
    label: PCWSTR,
    value: PCWSTR,
) -> HotkeyRowSpec {
    let label_spec = ControlSpec {
        ex_style: WINDOW_EX_STYLE(0),
        class: w!("STATIC"),
        text: label,
        style: WS_CHILD | WS_VISIBLE,
        rect: RectI::new(x, y + 3, w_label, 18),
        menu: None,
    };

    let edit_spec = ControlSpec {
        ex_style: WS_EX_CLIENTEDGE,
        class: w!("EDIT"),
        text: value,
        style: ws_i32(WS_CHILD | WS_VISIBLE, ES_READONLY),
        rect: RectI::new(x + w_label + 8, y, w_edit, 22),
        menu: None,
    };

    HotkeyRowSpec {
        label: label_spec,
        edit: edit_spec,
    }
}

pub fn create_controls(hwnd: HWND, state: &mut AppState) -> windows::core::Result<()> {
    // Determine client size for possible future dynamic layout
    let (_w, _h) = unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        (rc.right - rc.left, rc.bottom - rc.top)
    };

    let l = Layout::new();

    let left_x = l.left_x();
    let top_y = l.top_y();

    let right_x = l.right_x();
    let group_h = l.group_h();

    let group_w_left = l.group_w_left();
    let group_w_right = l.group_w_right();

    // Settings group box
    let _grp_settings = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Settings"),
            style: ws_i32(WS_CHILD | WS_VISIBLE, BS_GROUPBOX),
            rect: RectI::new(left_x, top_y, group_w_left, group_h),
            menu: None,
        },
    )?;

    state.checkboxes.autostart = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Start on startup"),
            style: ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            rect: RectI::new(left_x + 12, top_y + 28, group_w_left - 24, 20),
            menu: ControlId::Autostart.hmenu(),
        },
    )?;

    state.checkboxes.tray = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Show tray icon"),
            style: ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            rect: RectI::new(left_x + 12, top_y + 52, group_w_left - 24, 20),
            menu: ControlId::Tray.hmenu(),
        },
    )?;

    let _lbl_delay = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("STATIC"),
            text: w!("Delay before switching:"),
            style: WS_CHILD | WS_VISIBLE,
            rect: RectI::new(left_x + 12, top_y + 82, group_w_left - 24, 18),
            menu: None,
        },
    )?;

    state.edits.delay_ms = create(
        hwnd,
        ControlSpec {
            ex_style: WS_EX_CLIENTEDGE,
            class: w!("EDIT"),
            text: w!("100"),
            style: ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, ES_NUMBER),
            rect: RectI::new(left_x + 12, top_y + 104, 60, 22),
            menu: ControlId::DelayMs.hmenu(),
        },
    )?;

    let _lbl_ms = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("STATIC"),
            text: w!("ms"),
            style: WS_CHILD | WS_VISIBLE,
            rect: RectI::new(left_x + 78, top_y + 107, 24, 18),
            menu: None,
        },
    )?;

    // Hotkeys group box
    let _grp_hotkeys = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Hotkeys"),
            style: ws_i32(WS_CHILD | WS_VISIBLE, BS_GROUPBOX),
            rect: RectI::new(right_x, top_y, group_w_right, group_h),
            menu: None,
        },
    )?;

    // Hotkey rows
    let hx = right_x + 12;
    let mut hy = top_y + 28;
    let w_label = 130;
    let w_edit = group_w_right - 12 - 12 - w_label - 8;

    {
        let row = hotkey_row_spec(hx, hy, w_label, w_edit, w!("Convert last word:"), w!(""));
        let _ = create(hwnd, row.label)?;
        state.hotkeys.last_word = create(hwnd, row.edit)?;
        hy += 28;
    }

    {
        let row = hotkey_row_spec(hx, hy, w_label, w_edit, w!("Pause:"), w!(""));
        let _ = create(hwnd, row.label)?;
        state.hotkeys.pause = create(hwnd, row.edit)?;
        hy += 28;
    }

    {
        let row = hotkey_row_spec(hx, hy, w_label, w_edit, w!("Convert selection:"), w!(""));
        let _ = create(hwnd, row.label)?;
        state.hotkeys.selection = create(hwnd, row.edit)?;
        hy += 28;
    }

    {
        let row = hotkey_row_spec(
            hx,
            hy,
            w_label,
            w_edit,
            w!("Switch keyboard layout:"),
            w!(""),
        );
        let _ = create(hwnd, row.label)?;
        state.hotkeys.switch_layout = create(hwnd, row.edit)?;
    }

    // Buttons
    let btn_y = top_y + group_h + 10;
    let btn_h = 28;
    let btn_style = WS_CHILD | WS_VISIBLE | WS_TABSTOP;

    state.buttons.exit = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Exit"),
            style: btn_style,
            rect: RectI::new(left_x + 12, btn_y, 110, btn_h),
            menu: ControlId::Exit.hmenu(),
        },
    )?;

    state.buttons.apply = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Apply"),
            style: btn_style,
            rect: RectI::new(right_x + 40, btn_y, 90, btn_h),
            menu: ControlId::Apply.hmenu(),
        },
    )?;

    state.buttons.cancel = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Cancel"),
            style: btn_style,
            rect: RectI::new(right_x + 140, btn_y, 90, btn_h),
            menu: ControlId::Cancel.hmenu(),
        },
    )?;

    unsafe {
        let _ = SetWindowTextW(state.buttons.apply, w!("Apply"));
    }

    Ok(())
}
