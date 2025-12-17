#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, RECT,
            WPARAM,
        },
        Graphics::Gdi::{CreateFontIndirectW, DeleteObject, HFONT, HGDIOBJ, LOGFONTW},
        System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW},
        UI::{
            Controls::{
                ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx, SetWindowTheme,
            },
            WindowsAndMessaging::{
                AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
                DispatchMessageW, GetMessageW, GetSystemMetrics, MSG, NONCLIENTMETRICSW,
                RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SPI_GETNONCLIENTMETRICS, SW_SHOW,
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageW, ShowWindow,
                SystemParametersInfoW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_SETFONT,
                WNDCLASSW, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
            },
        },
    },
    core::{Error, HRESULT, PCWSTR, Result, w},
};

fn ws_i32(base: WINDOW_STYLE, extra: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(base.0 | extra as u32)
}

unsafe fn init_visuals() {
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES,
    };
    let _ = unsafe { InitCommonControlsEx(&icc) };
}

unsafe fn create_message_font() -> windows::core::Result<HFONT> {
    let mut ncm = NONCLIENTMETRICSW::default();
    ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;

    (unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()
    });

    let lf: LOGFONTW = ncm.lfMessageFont;
    Ok(unsafe { CreateFontIndirectW(&lf) })
}

unsafe fn apply_modern_look(state: &AppState) {
    let handles = [
        state.chk_autostart,
        state.chk_tray,
        state.edit_delay_ms,
        state.edit_hotkey_last_word,
        state.edit_hotkey_pause,
        state.edit_hotkey_selection,
        state.edit_hotkey_switch_layout,
        state.btn_apply,
        state.btn_cancel,
        state.btn_exit,
    ];

    for h in handles {
        if h.0.is_null() {
            continue;
        }
        let _ = unsafe { SetWindowTheme(h, w!("Explorer"), None) };
        let _ = unsafe {
            SendMessageW(
                h,
                WM_SETFONT,
                Some(WPARAM(state.font.0 as usize)),
                Some(LPARAM(1)),
            )
        };
    }
}

fn main() -> Result<()> {
    let _guard = single_instance_guard()?;

    unsafe {
        init_visuals();

        let class_name = w!("RustSwitcherMainWindow");

        let hinstance = GetModuleHandleW(PCWSTR::null())?.into();

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            lpszClassName: class_name,
            hInstance: hinstance,
            ..Default::default()
        };

        if RegisterClassW(&wc) == 0 {
            return Err(last_error());
        }

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

        let x = (GetSystemMetrics(SM_CXSCREEN) - window_w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - window_h) / 2;

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

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        loop {
            let r = GetMessageW(&mut msg, None, 0, 0);
            if r.0 == -1 {
                return Err(last_error());
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

fn last_error() -> Error {
    unsafe { Error::from_hresult(HRESULT::from_win32(GetLastError().0)) }
}

fn single_instance_guard() -> Result<SingleInstanceGuard> {
    unsafe {
        let name = w!("Global\\RustSwitcher_SingleInstance");
        let h = CreateMutexW(None, false, PCWSTR(name.as_ptr()))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            std::process::exit(0);
        }
        Ok(SingleInstanceGuard(h))
    }
}

struct SingleInstanceGuard(HANDLE);

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::{
        BN_CLICKED, DestroyWindow, GWLP_USERDATA, SetWindowLongPtrW, WM_COMMAND, WM_CREATE,
        WM_DESTROY,
    };

    const WM_NCDESTROY: u32 = 0x0082;

    unsafe fn loword(v: usize) -> u16 {
        (v & 0xffff) as u16
    }
    unsafe fn hiword(v: usize) -> u16 {
        ((v >> 16) & 0xffff) as u16
    }

    match msg {
        WM_CREATE => unsafe {
            let mut state = Box::new(AppState::default());

            if let Err(_) = create_controls(hwnd, &mut state) {
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            }

            match create_message_font() {
                Ok(font) => state.font = font,
                Err(_) => state.font = HFONT::default(),
            }

            if !state.font.0.is_null() {
                apply_modern_look(&state);
            }

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
            LRESULT(0)
        },

        WM_COMMAND => unsafe {
            let id = loword(w.0);
            let notif = hiword(w.0);

            if u32::from(notif) == BN_CLICKED {
                match id as i32 {
                    ID_EXIT => {
                        let _ = DestroyWindow(hwnd);
                        return LRESULT(0);
                    }
                    ID_APPLY => {
                        // TODO: прочитать значения из контролов и сохранить config + применить
                        return LRESULT(0);
                    }
                    ID_CANCEL => {
                        // TODO: откатить изменения в контролах (или просто hide окно, когда будет tray)
                        return LRESULT(0);
                    }
                    _ => {}
                }
            }

            LRESULT(0)
        },

        WM_DESTROY => unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            LRESULT(0)
        },

        WM_NCDESTROY => unsafe {
            let p = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            ) as *mut AppState;
            if !p.is_null() {
                if !(*p).font.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ((*p).font.0));
                }

                drop(Box::from_raw(p));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }

            LRESULT(0)
        },

        _ => unsafe { DefWindowProcW(hwnd, msg, w, LPARAM(l.0)) },
    }
}

#[derive(Default)]
struct AppState {
    font: HFONT,

    chk_autostart: HWND,
    chk_tray: HWND,
    edit_delay_ms: HWND,

    edit_hotkey_last_word: HWND,
    edit_hotkey_pause: HWND,
    edit_hotkey_selection: HWND,
    edit_hotkey_switch_layout: HWND,

    btn_apply: HWND,
    btn_cancel: HWND,
    btn_exit: HWND,
}

const ID_AUTOSTART: i32 = 1001;
const ID_TRAY: i32 = 1002;
const ID_DELAY_MS: i32 = 1003;

const ID_APPLY: i32 = 1101;
const ID_CANCEL: i32 = 1102;
const ID_EXIT: i32 = 1103;

fn create_controls(hwnd: HWND, state: &mut AppState) -> windows::core::Result<()> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::CreateWindowExW;
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    use windows::Win32::UI::WindowsAndMessaging::SetWindowTextW;
    use windows::Win32::UI::WindowsAndMessaging::{
        BS_AUTOCHECKBOX, BS_GROUPBOX, ES_NUMBER, ES_READONLY, WS_EX_CLIENTEDGE,
    };
    use windows::core::PCWSTR;
    use windows::core::w;

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{WS_CHILD, WS_TABSTOP, WS_VISIBLE};
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

        // Groupbox: Settings
        let _grp_settings = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
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

        state.chk_autostart = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Start on startup"),
            ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            left_x + 12,
            top_y + 28,
            group_w_left - 24,
            20,
            Some(hwnd),
            Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                ID_AUTOSTART as *mut _,
            )),
            None,
            None,
        )?;

        state.chk_tray = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Show tray icon"),
            ws_i32(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            left_x + 12,
            top_y + 52,
            group_w_left - 24,
            20,
            Some(hwnd),
            Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                ID_TRAY as *mut _,
            )),
            None,
            None,
        )?;

        let _lbl_delay = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
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
            Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                ID_DELAY_MS as *mut _,
            )),
            None,
            None,
        )?;

        let _lbl_ms = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
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

        state.btn_exit = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Exit program"),
            ws_i32(WS_CHILD | WS_VISIBLE, ES_READONLY),
            left_x + 12,
            top_y + group_h - 40,
            110,
            26,
            Some(hwnd),
            Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                ID_EXIT as *mut _,
            )),
            None,
            None,
        )?;

        // Groupbox: Hotkeys
        let _grp_hotkeys = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
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
            use windows::Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, ES_READONLY, WS_CHILD, WS_EX_CLIENTEDGE, WS_VISIBLE,
            };

            unsafe {
                let _lbl = CreateWindowExW(
                    windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
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

        // Parent for child windows is main hwnd, so coordinates are in client area.
        let hx = right_x + 12;
        let mut hy = top_y + 28;
        let w_label = 130;
        let w_edit = group_w_right - 12 - 12 - w_label - 8;

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

        // Bottom buttons: Apply / Cancel
        state.btn_apply = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Apply"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            right_x + 40,
            top_y + group_h + 10,
            90,
            28,
            Some(hwnd),
            Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                ID_APPLY as *mut _,
            )),
            None,
            None,
        )?;

        state.btn_cancel = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            w!("Cancel"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            right_x + 140,
            top_y + group_h + 10,
            90,
            28,
            Some(hwnd),
            Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                ID_CANCEL as *mut _,
            )),
            None,
            None,
        )?;

        // Optional: фокус по умолчанию на Apply
        let _ = SetWindowTextW(state.btn_apply, w!("Apply"));

        Ok(())
    }
}
