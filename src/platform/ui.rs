pub mod error_notifier;
pub mod geom;
pub mod info_notifier;
pub mod notify;
pub mod themes;

use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        System::SystemServices::SS_RIGHT,
        UI::WindowsAndMessaging::{
            BS_AUTOCHECKBOX, BS_OWNERDRAW, CreateWindowExW, ES_NUMBER, ES_READONLY, GetClientRect,
            SetWindowTextW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP,
            WS_VISIBLE,
        },
    },
    core::{PCWSTR, w},
};

use crate::{
    app::{AppState, ControlId},
    platform::ui::geom::{Layout, RectI},
    utils::helpers::ws_i32,
};

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
    menu: Option<windows::Win32::UI::WindowsAndMessaging::HMENU>,
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
        style: ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, ES_READONLY),
        rect: RectI::new(x + w_label + 8, y, w_edit, 22),
        menu,
    };

    HotkeyRowSpec {
        label: label_spec,
        edit: edit_spec,
    }
}

struct UiLayout {
    left_x: i32,
    right_x: i32,
    top_y: i32,
    group_h: i32,
    group_w_left: i32,
    group_w_right: i32,
}

impl UiLayout {
    fn new(client_w: i32) -> Self {
        let l = Layout::new(client_w);
        Self {
            left_x: l.left_x(),
            right_x: l.right_x(),
            top_y: l.top_y(),
            group_h: l.group_h(),
            group_w_left: l.group_w_left(),
            group_w_right: l.group_w_right(),
        }
    }
}

pub fn create_controls(hwnd: HWND, state: &mut AppState) -> windows::core::Result<()> {
    let (client_w, _client_h) = debug_read_client_rect(hwnd);
    let l = UiLayout::new(client_w);

    create_settings_group(hwnd, state, &l)?;
    create_hotkeys_group(hwnd, state, &l)?;
    create_buttons(hwnd, state, &l)?;
    create_version_label(hwnd, &l, client_w)?;

    Ok(())
}

fn create_version_label(hwnd: HWND, l: &UiLayout, client_w: i32) -> windows::core::Result<()> {
    let btn_y = l.top_y + l.group_h + 10;

    let version_w = 90;
    let version_h = 18;
    let version_x = client_w - l.left_x - version_w;
    let version_y = btn_y + 7;

    let h = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("STATIC"),
            text: w!(""),
            style: ws_i32(WS_CHILD | WS_VISIBLE, SS_RIGHT.0 as i32),
            rect: RectI::new(version_x, version_y, version_w, version_h),
            menu: None,
        },
    )?;

    let text = format!("v{}", env!("CARGO_PKG_VERSION"));
    crate::utils::helpers::set_edit_text(h, &text)?;

    Ok(())
}

fn debug_read_client_rect(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &raw mut rc);
        (rc.right - rc.left, rc.bottom - rc.top)
    }
}

fn create_settings_group(
    hwnd: HWND,
    state: &mut AppState,
    l: &UiLayout,
) -> windows::core::Result<()> {
    let left_x = l.left_x;
    let top_y = l.top_y;

    // let _grp_settings = create(
    //     hwnd,
    //     ControlSpec {
    //         ex_style: WINDOW_EX_STYLE(0),
    //         class: w!("BUTTON"),
    //         text: w!("Settings"),
    //         style: ws_i32(WS_CHILD | WS_VISIBLE, BS_GROUPBOX),
    //         rect: RectI::new(left_x, top_y, l.group_w_left, l.group_h),
    //         menu: None,
    //     },
    // )?;

    state.checkboxes.autostart = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Start on startup"),
            style: ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            rect: RectI::new(left_x + 12, top_y + 28, l.group_w_left - 24, 20),
            menu: Some(ControlId::Autostart.hmenu()),
        },
    )?;

    state.checkboxes.start_minimized = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Start minimized"),
            style: ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            rect: RectI::new(left_x + 12, top_y + 52, l.group_w_left - 24, 20),
            menu: Some(ControlId::StartMinimized.hmenu()),
        },
    )?;

    state.checkboxes.theme_dark = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Dark theme"),
            style: ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            rect: RectI::new(left_x + 12, top_y + 76, l.group_w_left - 24, 20),
            menu: Some(ControlId::DarkTheme.hmenu()),
        },
    )?;

    let _lbl_delay = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("STATIC"),
            text: w!("Delay before switching:"),
            style: WS_CHILD | WS_VISIBLE,
            rect: RectI::new(left_x + 12, top_y + 104, l.group_w_left - 24, 18),
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
            rect: RectI::new(left_x + 12, top_y + 126, 60, 22),
            menu: Some(ControlId::DelayMs.hmenu()),
        },
    )?;

    let _lbl_ms = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("STATIC"),
            text: w!("ms"),
            style: WS_CHILD | WS_VISIBLE,
            rect: RectI::new(left_x + 78, top_y + 129, 24, 18),
            menu: None,
        },
    )?;

    Ok(())
}

fn create_hotkeys_group(
    hwnd: HWND,
    state: &mut AppState,
    l: &UiLayout,
) -> windows::core::Result<()> {
    let root = HotkeysGroupLayout::new(l);

    create_hotkey_rows(hwnd, state, &root)?;

    Ok(())
}

#[derive(Clone, Copy)]
struct HotkeysGroupLayout {
    hx: i32,
    hy0: i32,
    w_label: i32,
    w_edit: i32,
}

impl HotkeysGroupLayout {
    fn new(l: &UiLayout) -> Self {
        let right_x = l.right_x;
        let top_y = l.top_y;

        let group_w = l.group_w_right;

        let hx = right_x + 12;
        let hy0 = top_y + 28;

        let w_label = 130;
        let w_edit = group_w - 12 - 12 - w_label - 8;

        Self {
            hx,
            hy0,
            w_label,
            w_edit,
        }
    }
}

// fn create_hotkeys_groupbox(hwnd: HWND, g: &HotkeysGroupLayout) -> windows::core::Result<()> {
//     // let _ = create(
//     //     hwnd,
//     //     ControlSpec {
//     //         ex_style: WINDOW_EX_STYLE(0),
//     //         class: w!("BUTTON"),
//     //         text: w!("Hotkeys"),
//     //         style: ws_i32(WS_CHILD | WS_VISIBLE, BS_GROUPBOX),
//     //         rect: RectI::new(g.right_x, g.top_y, g.group_w, g.group_h),
//     //         menu: None,
//     //     },
//     // )?;
//     Ok(())
// }

fn create_hotkey_rows(
    hwnd: HWND,
    state: &mut AppState,
    g: &HotkeysGroupLayout,
) -> windows::core::Result<()> {
    let mut hy = g.hy0;

    state.hotkeys.last_word = create_hotkey_row(
        hwnd,
        g.hx,
        hy,
        g.w_label,
        g.w_edit,
        w!("Convert last word:"),
        Some(ControlId::HotkeyLastWord.hmenu()),
    )?;
    hy += 28;

    state.hotkeys.pause = create_hotkey_row(
        hwnd,
        g.hx,
        hy,
        g.w_label,
        g.w_edit,
        w!("Autoconvert pause:"),
        Some(ControlId::HotkeyPause.hmenu()),
    )?;
    hy += 28;

    state.hotkeys.selection = create_hotkey_row(
        hwnd,
        g.hx,
        hy,
        g.w_label,
        g.w_edit,
        w!("Convert selection:"),
        Some(ControlId::HotkeySelection.hmenu()),
    )?;
    hy += 28;

    state.hotkeys.switch_layout = create_hotkey_row(
        hwnd,
        g.hx,
        hy,
        g.w_label,
        g.w_edit,
        w!("Switch keyboard layout:"),
        Some(ControlId::HotkeySwitchLayout.hmenu()),
    )?;

    Ok(())
}

fn create_hotkey_row(
    hwnd: HWND,
    x: i32,
    y: i32,
    w_label: i32,
    w_edit: i32,
    label: PCWSTR,
    menu: Option<windows::Win32::UI::WindowsAndMessaging::HMENU>,
) -> windows::core::Result<HWND> {
    let row = hotkey_row_spec(x, y, w_label, w_edit, label, w!(""), menu);
    let _ = create(hwnd, row.label)?;
    create(hwnd, row.edit)
}

fn create_buttons(hwnd: HWND, state: &mut AppState, l: &UiLayout) -> windows::core::Result<()> {
    let btn_y = l.top_y + l.group_h + 10;
    let btn_h = 28;
    let btn_style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32);

    state.buttons.exit = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Exit"),
            style: btn_style,
            rect: RectI::new(l.left_x + 12, btn_y, 110, btn_h),
            menu: Some(ControlId::Exit.hmenu()),
        },
    )?;

    state.buttons.apply = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Apply"),
            style: btn_style,
            rect: RectI::new(l.right_x + 40, btn_y, 90, btn_h),
            menu: Some(ControlId::Apply.hmenu()),
        },
    )?;

    state.buttons.cancel = create(
        hwnd,
        ControlSpec {
            ex_style: WINDOW_EX_STYLE(0),
            class: w!("BUTTON"),
            text: w!("Cancel"),
            style: btn_style,
            rect: RectI::new(l.right_x + 140, btn_y, 90, btn_h),
            menu: Some(ControlId::Cancel.hmenu()),
        },
    )?;

    unsafe {
        let _ = SetWindowTextW(state.buttons.apply, w!("Apply"));
    }

    Ok(())
}
