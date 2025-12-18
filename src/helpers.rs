//! Helper routines for interacting with the Windows API.  These
//! functions wrap common patterns such as combining style flags or
//! retrieving the last OS error. A RAII guard is provided for
//! enforcing a single application instance.

use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, LPARAM, WPARAM};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, HWND};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, GetWindowTextLengthW, GetWindowTextW, SendMessageW, SetWindowTextW,
    WINDOW_STYLE,
};
use windows::core::{Error, HRESULT, Result};
use windows::core::{PCWSTR, w};

/// Combine a base `WINDOW_STYLE` with an additional integer flag.
///
/// The Windows API often exposes style bits as separate types (e.g.
/// `WINDOW_STYLE` for window styles) while certain flags are defined
/// as plain integers.  This helper merges the two into a new
/// `WINDOW_STYLE` by OR‑ing the underlying values.
pub const fn ws_i32(base: WINDOW_STYLE, extra: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(base.0 | extra as u32)
}

/// Retrieve the last OS error as a `windows::core::Error`.
///
/// Calling Windows API functions directly leaves any error details
/// accessible only through `GetLastError`.  Converting the return
/// value into an `Error` makes it easier to propagate failures from
/// leaf functions up the call chain.
pub fn last_error() -> Error {
    Error::from_hresult(HRESULT::from_win32(unsafe { GetLastError() }.0))
}

/// RAII guard which ensures that only a single instance of the
/// application is running at a time.
///
/// On creation the guard attempts to create a named mutex.  If
/// another process already owns the mutex the current process will
/// immediately terminate.  When the guard is dropped the mutex
/// handle is released.
pub struct SingleInstanceGuard(pub HANDLE);

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            // Ignoring the return value is fine – if closing the handle
            // fails there isn't anything sensible we can do anyway.
            let _ = CloseHandle(self.0);
        }
    }
}

/// Acquire the single instance mutex.
///
/// If the mutex already exists the process is terminated.  On
/// success a `SingleInstanceGuard` is returned which releases the
/// mutex when dropped.
pub fn single_instance_guard() -> Result<SingleInstanceGuard> {
    unsafe {
        let name = w!("Global\\RustSwitcher_SingleInstance");
        let h = CreateMutexW(None, false, PCWSTR(name.as_ptr()))?;

        if GetLastError() == ERROR_ALREADY_EXISTS {
            std::process::exit(0);
        }

        Ok(SingleInstanceGuard(h))
    }
}

/// Extract the low 16 bits from a 32‑bit packed parameter.
///
/// In the Windows message system a single `WPARAM` or `LPARAM` often
/// encodes two separate 16‑bit values.  Use this helper to decode
/// the low word from such a parameter.
pub const fn loword(v: usize) -> u16 {
    (v & 0xffff) as u16
}

/// Extract the high 16 bits from a 32‑bit packed parameter.
///
/// See `loword` for more details.  This function shifts the input
/// right by 16 bits and masks off the low word to return the upper
/// portion of a packed parameter.
pub const fn hiword(v: usize) -> u16 {
    ((v >> 16) & 0xffff) as u16
}

pub fn default_window_pos(window_w: i32, window_h: i32) -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    #[cfg(debug_assertions)]
    {
        // 3x3 grid, cell 7 (left bottom), centered inside that cell
        let cell_w = sw / 3;
        let cell_h = sh / 3;

        let cx = cell_w / 2;
        let cy = cell_h * 2 + cell_h / 2;

        let mut x = cx - window_w / 2;
        let mut y = cy - window_h / 2;

        // Manual tweak for удобного места на твоем сетапе
        x -= 100;
        y -= 95;

        // Clamp to screen
        if x < 0 {
            x = 0;
        }
        if y < 0 {
            y = 0;
        }
        if x + window_w > sw {
            x = sw - window_w;
        }
        if y + window_h > sh {
            y = sh - window_h;
        }

        (x, y)
    }

    #[cfg(not(debug_assertions))]
    {
        let x = (sw - window_w) / 2;
        let y = (sh - window_h) / 2;
        (x, y)
    }
}

pub unsafe fn set_checkbox(hwnd: HWND, value: bool) {
    let v = if value { BST_CHECKED } else { BST_UNCHECKED };
    let _ = unsafe {
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            Some(WPARAM(v.0 as usize)),
            Some(LPARAM(0)),
        )
    };
}

pub unsafe fn get_checkbox(hwnd: HWND) -> bool {
    let r = unsafe { SendMessageW(hwnd, BM_GETCHECK, Some(WPARAM(0)), Some(LPARAM(0))) };
    r.0 as u32 == BST_CHECKED.0
}

pub unsafe fn set_edit_text(hwnd: HWND, s: &str) {
    let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(wide.as_ptr())) };
}

pub unsafe fn get_edit_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf: Vec<u16> = vec![0; (len as usize) + 1];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    let n = n.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

pub unsafe fn set_edit_u32(hwnd: HWND, value: u32) {
    unsafe { set_edit_text(hwnd, &value.to_string()) };
}

pub unsafe fn get_edit_u32(hwnd: HWND) -> Option<u32> {
    let s = unsafe { get_edit_text(hwnd) };
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    s.parse::<u32>().ok()
}
