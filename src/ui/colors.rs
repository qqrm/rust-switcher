use windows::Win32::{
    Foundation::{COLORREF, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{
        COLOR_WINDOW, COLOR_WINDOWTEXT, GetSysColor, GetSysColorBrush, HBRUSH, HDC, SetBkMode,
        SetTextColor, TRANSPARENT,
    },
};

/// Handles `WM_CTLCOLOR*` style messages by configuring the device context and returning a brush.
///
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
