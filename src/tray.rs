use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND},
        UI::{
            Shell::{
                NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_ERROR, NIM_ADD, NIM_DELETE, NIF_SHOWTIP, NIM_MODIFY, NIM_SETVERSION, NOTIFYICON_VERSION_4, 
                NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                GWLP_HINSTANCE, IMAGE_ICON, LR_SHARED, WM_APP, MF_SEPARATOR, MF_STRING, TPM_BOTTOMALIGN, TPM_NOANIMATION, TPM_RETURNCMD, TPM_RIGHTALIGN, TPM_RIGHTBUTTON,
                SetForegroundWindow, TrackPopupMenu, CreatePopupMenu, DestroyMenu, InsertMenuW, GetCursorPos, GetWindowLongPtrW, LoadImageW
            },
        },
    },
    core::PCWSTR,
};

pub const WM_APP_TRAY: u32 = WM_APP + 3;
const TRAY_UID: u32 = 1;
const ID_EXIT: u32 = 1001;
const ID_SHOW_HIDE: u32 = 1002;

pub fn show_tray_context_menu(hwnd: HWND, window_visible: bool) -> windows::core::Result<()> {
    unsafe {
        let hmenu = CreatePopupMenu()?;
        
        // Add "Show/Hide" item
        let show_hide_text = if window_visible {
            "Hide\0"
        } else {
            "Show\0"
        };
        let show_hide_text_vec = show_hide_text.encode_utf16().collect::<Vec<u16>>();
        InsertMenuW(
            hmenu,
            0,
            MF_STRING,
            ID_SHOW_HIDE as usize,
            PCWSTR(show_hide_text_vec.as_ptr()),
        )?;

        InsertMenuW(hmenu, 1, MF_SEPARATOR, 0, PCWSTR::null())?;

        // Add "Exit" item
        let exit_text = "Exit\0".encode_utf16().collect::<Vec<u16>>();
        InsertMenuW(
            hmenu,
            0,
            MF_STRING,
            ID_EXIT as usize,
            PCWSTR(exit_text.as_ptr()),
        )?;
        
        // Add separator
        InsertMenuW(hmenu, 1, MF_SEPARATOR, 0, PCWSTR::null())?;
        
        // Get cursor position
        let mut pt = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt)?;
        
        // Show menu at cursor position
        let _fg = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            hmenu,
            TPM_RETURNCMD | TPM_BOTTOMALIGN | TPM_RIGHTALIGN | TPM_NOANIMATION | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
        
        let _destroy = DestroyMenu(hmenu);
        
        // Handle selection
        match cmd.0 as u32 {
            ID_SHOW_HIDE => {
                // Toggle window visibility
                let _current_style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
                );
                
                if window_visible {
                    // Hide window
                    let _show = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                    );
                } else {
                    // Show window
                    let _show = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::SW_SHOW,
                    );
                    let _fg = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
                }
            }
            ID_EXIT => {
                // Send WM_CLOSE to exit
                windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    Some(hwnd),
                    windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0),
                )?;
            }
            _ => {}
        }
        
        Ok(())
    }
}

fn fill_wide(dst: &mut [u16], s: &str) {
    if let Some((last, body)) = dst.split_last_mut() {
        for (d, ch) in body
            .iter_mut()
            .zip(s.encode_utf16().chain(std::iter::repeat(0)))
        {
            *d = ch;
        }
        *last = 0;
    }
}
fn shell_notify(
    action: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE,
    nid: &NOTIFYICONDATAW,
    what: &str,
) -> windows::core::Result<()> {
    unsafe {
        if Shell_NotifyIconW(action, nid).as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32), // E_FAIL
                format!("Shell_NotifyIconW returned FALSE: {}", what),
            ))
        }
    }
}

pub fn ensure_icon(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: core::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            ..Default::default()
        };

        nid.uCallbackMessage = WM_APP_TRAY;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;

        nid.hIcon = default_icon(hwnd)?;
        fill_wide(&mut nid.szTip, "RustSwitcher");

        // Shell_NotifyIconW может вернуть FALSE без last error.
        // Поэтому делаем add, а если не вышло, пробуем modify.
        if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            shell_notify(
                NIM_MODIFY,
                &nid,
                "ensure_icon: NIM_MODIFY after NIM_ADD failure",
            )?;
        }

        // Версия поведения
        //nid.uFlags = windows::Win32::UI::Shell::NOTIFY_ICON_DATA_FLAGS(0);
        nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;

        if !Shell_NotifyIconW(NIM_SETVERSION, &nid).as_bool() {
            // Это не критично для жизни, но пусть будет сигналом
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                "Shell_NotifyIconW returned FALSE: ensure_icon NIM_SETVERSION",
            ));
        }

        Ok(())
    }
}

fn balloon_common(
    hwnd: HWND,
    title: &str,
    text: &str,
    flags: u32,
    what: &str,
) -> windows::core::Result<()> {
    ensure_icon(hwnd)?;

    let mut nid = NOTIFYICONDATAW {
        cbSize: core::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };

    // Всегда шлем NIF_MESSAGE + callback, чтобы Explorer не терял связку
    nid.uCallbackMessage = WM_APP_TRAY;
    nid.uFlags = NIF_INFO | NIF_MESSAGE;
    nid.dwInfoFlags = windows::Win32::UI::Shell::NOTIFY_ICON_INFOTIP_FLAGS(flags);

    // Можно задать таймаут (Windows может игнорировать, но вреда нет)
    nid.Anonymous.uTimeout = 10_000;

    fill_wide(&mut nid.szInfoTitle, title);
    fill_wide(&mut nid.szInfo, text);

    // Первая попытка
    if unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() } {
        return Ok(());
    }

    // Самовосстановление: пересоздаем иконку и пробуем еще раз
    remove_icon(hwnd);
    ensure_icon(hwnd)?;

    shell_notify(NIM_MODIFY, &nid, what)
}

pub fn balloon_error(hwnd: HWND, title: &str, text: &str) -> windows::core::Result<()> {
    balloon_common(hwnd, title, text, NIIF_ERROR.0, "balloon_error: NIM_MODIFY")
}

pub fn balloon_info(hwnd: HWND, title: &str, text: &str) -> windows::core::Result<()> {
    balloon_common(
        hwnd,
        title,
        text,
        windows::Win32::UI::Shell::NIIF_INFO.0,
        "balloon_info: NIM_MODIFY",
    )
}

unsafe fn default_icon(
    hwnd: HWND,
) -> windows::core::Result<windows::Win32::UI::WindowsAndMessaging::HICON> {
    let hinst = unsafe { GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) };
    let hinst = HINSTANCE(hinst as *mut core::ffi::c_void);

    let h = unsafe {
        LoadImageW(
            Some(hinst),
            #[allow(clippy::manual_dangling_ptr)]
            PCWSTR(1usize as *const u16),
            IMAGE_ICON,
            0,
            0,
            LR_SHARED,
        )
        .map(|h| windows::Win32::UI::WindowsAndMessaging::HICON(h.0))
    }?;

    Ok(h)
}

pub fn remove_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: core::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            ..Default::default()
        };

        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}
