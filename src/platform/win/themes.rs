use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::core::{BOOL, w};
use windows::Win32::Foundation::{HWND};
use super::state::with_state_mut_do;

// Define RECT structure
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

// Define RGNDATA structure (simplified - we only need a pointer to it)
#[repr(C)]
pub struct RGNDATA {
    _private: [u8; 0],  // Zero-sized type since we only pass null
}

#[link(name = "user32")]
unsafe extern "system" {
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;
    pub fn RedrawWindow(
        hWnd: HWND,
        lprcUpdate: *const RECT,
        hrgnUpdate: *const RGNDATA,
        flags: u32,
    ) -> BOOL;
}

const RDW_INVALIDATE: u32 = 0x0001;
const RDW_ALLCHILDREN: u32 = 0x0080;

pub fn set_window_theme(hwnd_main: HWND, current_theme_dark: bool) {
    crate::utils::helpers::debug_log(&format!("dark={current_theme_dark}"));
    
    //let hwnd_main = get_main_window_handle().unwrap();
    unsafe {
        if !current_theme_dark {
            let mut dark_mode: BOOL = BOOL(1);
    
            let _ = DwmSetWindowAttribute(
                hwnd_main,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &mut dark_mode as *mut _ as *const _,
                std::mem::size_of::<BOOL>() as u32,
            );
            
            let _ = SetWindowTheme(hwnd_main, w!("DarkMode_Explorer"), windows::core::PCWSTR::null());
            with_state_mut_do(hwnd_main, |state| {
                state.current_theme_dark = true;
            });
            crate::utils::helpers::debug_log(&format!("dark mode set to TRUE"));
        } else {
            // Revert to light mode
            let mut light_mode: BOOL = BOOL(0);
            let _ = DwmSetWindowAttribute(
                hwnd_main,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &mut light_mode as *mut _ as *const _,
                std::mem::size_of::<BOOL>() as u32,
            );
            let _ = SetWindowTheme(hwnd_main, w!("Explorer"), windows::core::PCWSTR::null());
            with_state_mut_do(hwnd_main, |state| {
                state.current_theme_dark = false;
            });
            crate::utils::helpers::debug_log(&format!("dark mode set to FALSE"));
        }

        // Force window repaint to apply the theme changes
        let _ = InvalidateRect(hwnd_main, std::ptr::null(), BOOL(1));
        let _ = UpdateWindow(hwnd_main);
        
        // Also redraw child controls
        let _ = RedrawWindow(
            hwnd_main, 
            std::ptr::null(), 
            std::ptr::null(), 
            RDW_INVALIDATE | RDW_ALLCHILDREN
        );
    }
}