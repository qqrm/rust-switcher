use windows::core::PCWSTR;

/// Convert a Win32 integer resource identifier (MAKEINTRESOURCEW-style) to `PCWSTR`.
///
/// Win32 APIs commonly accept either a string pointer or an integer resource id encoded as a
/// pointer value. The Windows API defines this convention; it is not a real dereferenceable
/// pointer.
///
/// Safety:
/// - The resulting `PCWSTR` must only be passed to Win32 APIs that explicitly document support
///   for integer resource identifiers (e.g. `LoadImageW`).
/// - It must never be dereferenced.
#[allow(clippy::manual_dangling_ptr)]
pub(crate) const fn make_int_resource(id: u16) -> PCWSTR {
    PCWSTR(id as usize as *const u16)
}
