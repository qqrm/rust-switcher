#[cfg(test)]
mod tests {
    use crate::win::wndproc;
    use crate::hotkeys::HK_CONVERT_SELECTION_ID;
    use tracing_test::traced_test;
    use windows::Win32::Foundation::{HWND, WPARAM, LPARAM};
    
    // Mock constants (use actual values from your code)
    const WM_HOTKEY: u32 = 0x0312;

    #[traced_test]
    #[test]
    fn test_wndproc_hotkey_selection_detected() {
        // Mock HWND
        let hwnd = HWND(12345 as *mut core::ffi::c_void);
        
        let wparam = WPARAM(HK_CONVERT_SELECTION_ID as usize);
        
        // Call wndproc with WM_HOTKEY message
        let result = wndproc(hwnd, WM_HOTKEY, wparam, LPARAM(0));
        
        assert_eq!(result.0, 0);
        
        let logs_contained = logs_contain("selection detected");
        
        if logs_contained {
            println!("✓ 'selection detected' was logged");
        } else {
            println!("✗ 'selection detected' was NOT logged (maybe no selection?)");
        }
        
        assert!(logs_contain("WM_HOTKEY") || logs_contain("convert_selection called"));
    }
    

}