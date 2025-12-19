//! Visual styling and common control initialization.
//!
//! Functions in this module deal with preparing Windows common
//! controls and applying theming information to the application's
//! controls.  Separating these routines keeps the window creation
//! logic in `win` clear of stylistic concerns.

use windows::Win32::Foundation::{E_FAIL, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateFontIndirectW, HFONT};
use windows::Win32::UI::Controls::{
    ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx, SetWindowTheme,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageW, SystemParametersInfoW, WM_SETFONT,
};
use windows::core::{BOOL, Result, w};

/// Register common control classes so that modern UI elements (e.g.
/// group boxes, push buttons) can be created.
///
/// This function should be called once before any controls are
/// instantiated.  Without it some controls may revert to legacy
/// appearances on older versions of Windows.
pub unsafe fn init_visuals() {
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES,
    };
    let _ = unsafe { InitCommonControlsEx(&icc) };
}

/// Create a font matching the system message font.
///
/// Many Windows applications use the message font for consistency.
/// This helper returns an `HFONT` handle which must be deleted
/// manually once it is no longer needed.  The caller is responsible
/// for freeing the returned font using `DeleteObject` on destruction.
pub unsafe fn create_message_font() -> Result<HFONT> {
    let mut non_client_metrics = NONCLIENTMETRICSW {
        cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };

    let pv_param = (&mut non_client_metrics as *mut NONCLIENTMETRICSW).cast::<core::ffi::c_void>();

    unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            non_client_metrics.cbSize,
            Some(pv_param),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
    }

    let font = unsafe { CreateFontIndirectW(&non_client_metrics.lfMessageFont) };
    if font.0.is_null() {
        return Err(windows::core::Error::from_hresult(E_FAIL));
    }

    Ok(font)
}

/// Apply a modern visual theme and the configured font to the
/// application's controls.
///
/// Iterates over every control stored in the `AppState` and calls
/// `SetWindowTheme` and `SendMessageW` with `WM_SETFONT` to ensure
/// consistent theming and typography.
pub unsafe fn apply_modern_look(hwnd: windows::Win32::Foundation::HWND, font: HFONT) {
    if font.0.is_null() {
        return;
    }

    // Main window too
    let _ = unsafe {
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        )
    };

    unsafe extern "system" fn enum_proc(
        child: windows::Win32::Foundation::HWND,
        l: LPARAM,
    ) -> BOOL {
        let font = HFONT(l.0 as *mut _);

        let _ = unsafe { SetWindowTheme(child, w!("Explorer"), None) };
        let _ = unsafe {
            SendMessageW(
                child,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            )
        };

        BOOL(1)
    }

    let _ = unsafe { EnumChildWindows(Some(hwnd), Some(enum_proc), LPARAM(font.0 as isize)) };
}
