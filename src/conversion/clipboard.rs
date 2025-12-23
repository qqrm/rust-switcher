use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
            OpenClipboard, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
};

const CF_UNICODETEXT_ID: u32 = 13;

struct ClipboardGuard;

impl ClipboardGuard {
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

pub fn get_unicode_text() -> Option<String> {
    let _clip = ClipboardGuard::open()?;

    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT_ID).ok()?;
        if handle.0.is_null() {
            return None;
        }

        let hglobal = HGLOBAL(handle.0);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            return None;
        }

        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        let slice = std::slice::from_raw_parts(ptr, len);
        let text = String::from_utf16_lossy(slice);

        let _ = GlobalUnlock(hglobal);
        Some(text)
    }
}

pub fn set_unicode_text(text: &str) -> bool {
    let _clip = match ClipboardGuard::open() {
        Some(g) => g,
        None => return false,
    };

    unsafe {
        let _ = EmptyClipboard();

        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);

        let bytes = units.len() * 2;

        let hmem = match GlobalAlloc(GMEM_MOVEABLE, bytes) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(error = ?e, "GlobalAlloc failed");
                return false;
            }
        };

        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            tracing::warn!("GlobalLock returned null");
            return false;
        }

        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
        let _ = GlobalUnlock(hmem);

        let handle = HANDLE(hmem.0);
        match SetClipboardData(CF_UNICODETEXT_ID, Some(handle)) {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(error = ?e, "SetClipboardData failed");
                false
            }
        }
    }
}

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
