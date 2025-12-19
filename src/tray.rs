use crate::helpers;

use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_ERROR, NIIF_USER, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWLP_HINSTANCE, GetWindowLongPtrW, IMAGE_ICON, LR_SHARED, LoadImageW, WM_APP,
};

use windows::core::PCWSTR;

pub const WM_APP_TRAY: u32 = WM_APP + 2;
const TRAY_UID: u32 = 1;

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
) -> windows::core::Result<()> {
    unsafe {
        match Shell_NotifyIconW(action, nid).ok() {
            Ok(()) => Ok(()),
            Err(e) => {
                if e.code() == windows::core::HRESULT(0) {
                    Err(windows::core::Error::new(
                        windows::core::HRESULT(0x80004005u32 as i32), // E_FAIL
                        "Shell_NotifyIconW failed",
                    ))
                } else {
                    Err(e)
                }
            }
        }
    }
}

unsafe fn default_icon(
    hwnd: HWND,
) -> windows::core::Result<windows::Win32::UI::WindowsAndMessaging::HICON> {
    let hinst = unsafe { GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) };
    let hinst = HINSTANCE(hinst as *mut core::ffi::c_void);

    let h = unsafe {
        LoadImageW(
            Some(hinst),
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
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_UID;
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}
pub fn ensure_icon(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_UID;
        nid.uCallbackMessage = WM_APP_TRAY;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;

        nid.hIcon = default_icon(hwnd)?;
        fill_wide(&mut nid.szTip, "RustSwitcher");

        // Try add, fall back to modify if the icon already exists.
        if let Err(e) = shell_notify(NIM_ADD, &nid) {
            let code = e.code();
            let already_exists_183 = windows::core::HRESULT::from_win32(183); // ERROR_ALREADY_EXISTS
            let file_exists_80 = windows::core::HRESULT::from_win32(80); // ERROR_FILE_EXISTS

            if code == already_exists_183 || code == file_exists_80 {
                shell_notify(NIM_MODIFY, &nid)?;
            } else {
                return Err(e);
            }
        }

        // Ensure modern behavior for notifications and callbacks.
        nid.uFlags = windows::Win32::UI::Shell::NOTIFY_ICON_DATA_FLAGS(0);
        nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;

        shell_notify(NIM_SETVERSION, &nid)
    }
}

pub fn balloon_error(hwnd: HWND, title: &str, text: &str) -> windows::core::Result<()> {
    ensure_icon(hwnd)?;

    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_UID;

    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_ERROR; // важно: без NIIF_USER

    fill_wide(&mut nid.szInfoTitle, title);
    fill_wide(&mut nid.szInfo, text);

    shell_notify(NIM_MODIFY, &nid)
}
