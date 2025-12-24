#[cfg(debug_assertions)]
pub fn push(
    hwnd: windows::Win32::Foundation::HWND,
    _state: &mut crate::app::AppState,
    title: &str,
    text: &str,
) {
    let _ = crate::tray::balloon_info(hwnd, title, text);
}
