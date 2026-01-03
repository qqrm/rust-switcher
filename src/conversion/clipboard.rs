use std::ptr::null_mut;

use windows::{
    Win32::{
        Foundation::{HANDLE, HGLOBAL},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
                GetClipboardSequenceNumber, OpenClipboard, SetClipboardData,
            },
            Memory::{
                GMEM_MOVEABLE, GlobalAlloc, GlobalFree, GlobalLock, GlobalSize, GlobalUnlock,
            },
            Ole::{IDataObject, OleGetClipboard, OleInitialize, OleSetClipboard, OleUninitialize},
        },
    },
    core::HRESULT,
};

/// Win32 clipboard format id for UTF 16 text (`CF_UNICODETEXT`).
const CF_UNICODETEXT_ID: u32 = 13;

/// RAII guard that opens the process clipboard on creation and closes it on drop.
///
/// Notes:
/// - Win32 clipboard is a global process shared resource. Opening may fail if another process
///   has it open at the moment.
/// - This guard always calls `CloseClipboard` in `Drop` even if operations inside fail.
struct ClipboardGuard;

impl ClipboardGuard {
    /// Opens the clipboard for the current process.
    ///
    /// Returns `None` if `OpenClipboard` fails (for example, clipboard is temporarily locked by
    /// another process).
    fn open() -> Option<Self> {
        unsafe { OpenClipboard(None).ok()? };
        Some(Self)
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// Reads current clipboard content as UTF 16 text (`CF_UNICODETEXT`) and returns it as `String`.
///
/// Returns:
/// - `Some(String)` if clipboard contains `CF_UNICODETEXT` and data can be locked and decoded.
/// - `None` if clipboard cannot be opened, format not available, global lock fails, or the handle is null.
///
/// Safety/FFI notes:
/// - `GetClipboardData` returns a handle owned by the clipboard. Do not free it.
/// - `GlobalLock` provides a pointer valid until `GlobalUnlock`.
/// - Text is expected to be NUL terminated UTF 16; we scan until the first NUL.
///
/// Possible improvements:
/// - Use `GlobalUnlock` return value + `GetLastError` to detect unlock errors (rarely needed).
/// - Reject very large payloads to avoid scanning unbounded memory if clipboard data is malformed
///   (defensive bound, for example `1_048_576` UTF 16 units).
pub fn get_unicode_text() -> Option<String> {
    let _clip = ClipboardGuard::open()?;

    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT_ID).ok()?;
        if handle.0.is_null() {
            return None;
        }

        let hglobal = HGLOBAL(handle.0);
        let size = GlobalSize(hglobal);
        if size == 0 {
            return None;
        }

        let max_units = size / std::mem::size_of::<u16>();
        if max_units == 0 {
            return None;
        }

        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            return None;
        }

        // Scan for NUL terminator.
        let mut len = 0usize;
        while len < max_units && *ptr.add(len) != 0 {
            len += 1;
        }

        let slice = std::slice::from_raw_parts(ptr, len);
        let text = String::from_utf16_lossy(slice);

        let _ = GlobalUnlock(hglobal);
        Some(text)
    }
}

struct GlobalMem {
    handle: HGLOBAL,
    owned: bool,
}

impl GlobalMem {
    fn new(handle: HGLOBAL) -> Self {
        Self {
            handle,
            owned: true,
        }
    }

    fn disarm(&mut self) {
        self.owned = false;
    }
}

impl Drop for GlobalMem {
    fn drop(&mut self) {
        if self.owned && !self.handle.0.is_null() {
            unsafe {
                let _ = GlobalFree(self.handle);
            }
        }
    }
}

struct OleGuard {
    active: bool,
}

impl OleGuard {
    fn init() -> Result<Option<Self>, windows::core::Error> {
        match unsafe { OleInitialize(null_mut()) } {
            Ok(_) => Ok(Some(Self { active: true })),
            Err(e) if e.code() == HRESULT(0x8001_0106_u32 as i32) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Drop for OleGuard {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                OleUninitialize();
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ClipboardFormatData {
    format: u32,
    data: Vec<u8>,
}

pub enum ClipboardSnapshot {
    Ole {
        data_object: IDataObject,
        _ole: Option<OleGuard>,
    },
    Formats {
        formats: Vec<ClipboardFormatData>,
    },
}

/// Captures a snapshot of clipboard contents.
///
/// Returns `None` if the clipboard cannot be opened or captured.
pub fn snapshot() -> Option<ClipboardSnapshot> {
    if let Ok(ole) = OleGuard::init() {
        if let Ok(data_object) = unsafe { OleGetClipboard() } {
            return Some(ClipboardSnapshot::Ole {
                data_object,
                _ole: ole,
            });
        }
    }

    let _clip = ClipboardGuard::open()?;

    let mut formats = Vec::new();
    let mut format = 0u32;

    loop {
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }

        let handle = match unsafe { GetClipboardData(format) } {
            Ok(h) if !h.0.is_null() => h,
            _ => continue,
        };

        let hglobal = HGLOBAL(handle.0);
        let size = unsafe { GlobalSize(hglobal) };
        if size == 0 {
            continue;
        }

        let ptr = unsafe { GlobalLock(hglobal) };
        if ptr.is_null() {
            continue;
        }

        let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, size) };
        let data = slice.to_vec();
        let _ = unsafe { GlobalUnlock(hglobal) };

        formats.push(ClipboardFormatData { format, data });
    }

    if formats.is_empty() {
        return None;
    }

    Some(ClipboardSnapshot::Formats { formats })
}

/// Restores a clipboard snapshot.
///
/// Returns `true` when all formats were restored successfully.
pub fn restore_snapshot(snapshot: &ClipboardSnapshot) -> bool {
    match snapshot {
        ClipboardSnapshot::Ole { data_object, .. } => unsafe {
            OleSetClipboard(data_object).is_ok()
        },
        ClipboardSnapshot::Formats { formats } => {
            let Some(_clip) = ClipboardGuard::open() else {
                return false;
            };

            unsafe {
                let _ = EmptyClipboard();
            }

            formats.iter().all(|entry| {
                let Ok(hmem) = (unsafe { GlobalAlloc(GMEM_MOVEABLE, entry.data.len()) }) else {
                    tracing::warn!("GlobalAlloc failed while restoring clipboard");
                    return false;
                };

                let mut mem = GlobalMem::new(hmem);

                let ptr = unsafe { GlobalLock(hmem) };
                if ptr.is_null() {
                    tracing::warn!("GlobalLock returned null while restoring clipboard");
                    return false;
                }

                unsafe {
                    std::ptr::copy_nonoverlapping(
                        entry.data.as_ptr(),
                        ptr.cast(),
                        entry.data.len(),
                    );
                }
                let _ = unsafe { GlobalUnlock(hmem) };

                let handle = HANDLE(hmem.0);
                match unsafe { SetClipboardData(entry.format, Some(handle)) } {
                    Ok(_) => {
                        mem.disarm();
                        true
                    }
                    Err(e) => {
                        tracing::warn!(error = ?e, "SetClipboardData failed while restoring clipboard");
                        false
                    }
                }
            })
        }
    }
}

/// Replaces clipboard content with UTF 16 text (`CF_UNICODETEXT`).
///
/// Returns `true` on success, `false` on failure.
///
/// Operational details:
/// - Opens clipboard (fails fast if clipboard is locked).
/// - Empties clipboard.
/// - Allocates a movable global memory block (`GMEM_MOVEABLE`) and copies NUL terminated UTF 16.
/// - Transfers ownership of the memory block to the clipboard via `SetClipboardData`.
///
/// Safety/FFI notes:
/// - After successful `SetClipboardData`, the system owns the `HGLOBAL`.
/// - On failure after allocation, this code currently leaks the allocated `HGLOBAL` because it is
///   neither freed nor transferred. If you want strict correctness, wrap the allocation into a small
///   RAII type that calls `GlobalFree` unless `SetClipboardData` succeeds.
///
/// Idiomatic improvements recommended:
/// - Return `windows::core::Result<()>` instead of `bool` to keep error context.
/// - Add `GlobalFree` on failure paths.
/// - Use `size_of::<u16>()` rather than hardcoded `2`.
pub fn set_unicode_text(text: &str) -> bool {
    let Some(_clip) = ClipboardGuard::open() else {
        return false;
    };

    unsafe {
        let _ = EmptyClipboard();

        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);

        let bytes = units.len() * std::mem::size_of::<u16>();

        let hmem = match GlobalAlloc(GMEM_MOVEABLE, bytes) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(error = ?e, "GlobalAlloc failed");
                return false;
            }
        };

        let mut mem = GlobalMem::new(hmem);

        let ptr = GlobalLock(hmem).cast::<u16>();
        if ptr.is_null() {
            tracing::warn!("GlobalLock returned null");
            return false;
        }

        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
        let _ = GlobalUnlock(hmem);

        let handle = HANDLE(hmem.0);
        match SetClipboardData(CF_UNICODETEXT_ID, Some(handle)) {
            Ok(_) => {
                mem.disarm();
                true
            }
            Err(e) => {
                tracing::warn!(error = ?e, "SetClipboardData failed");
                false
            }
        }
    }
}

/// Polls the clipboard sequence number until it changes or tries are exhausted.
///
/// Parameters:
/// - `before`: the sequence number captured earlier (via `GetClipboardSequenceNumber`).
/// - `tries`: how many polling attempts to make.
/// - `sleep_ms`: delay between polls.
///
/// Returns:
/// - `true` if sequence number changed.
/// - `false` if it did not change within the allotted attempts.
///
/// Notes:
/// - `GetClipboardSequenceNumber` is cheap and does not require opening the clipboard.
/// - This is a heuristic for "clipboard content changed", not a guarantee that desired format exists.
///
/// Possible improvements:
/// - Add an early return if `tries == 0`.
/// - Clamp `sleep_ms` to a sane minimum or maximum to avoid too tight loops or huge stalls.
pub fn wait_change(before: u32, tries: usize, sleep_ms: u64) -> bool {
    for _ in 0..tries {
        let now = unsafe { GetClipboardSequenceNumber() };
        if now != before {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    }
    false
}
