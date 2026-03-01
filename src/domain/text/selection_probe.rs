use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RPC_E_CHANGED_MODE, WPARAM},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
            },
            Controls::EM_GETSEL,
            WindowsAndMessaging::{
                GUITHREADINFO, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo,
                GetWindowThreadProcessId, SendMessageW, WM_GETTEXT, WM_GETTEXTLENGTH,
            },
        },
    },
    core::PWSTR,
};

#[derive(Debug)]
pub(super) enum SelectionProbe {
    NoSelection,
    SelectionText(String),
    SelectionPresentButIneligible,
    SelectionPresentButUnreadable(windows::core::Error),
    Unsupported,
}

/// Attempts to read the current selection from the focused control via UI Automation.
///
/// This probe never sends synthetic input.
pub(super) fn probe_selection_uia(max_chars: usize) -> SelectionProbe {
    struct CoUninitGuard(bool);

    impl Drop for CoUninitGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    let should_uninit = match unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok() {
        Ok(()) => true,
        Err(e) if e.code() == RPC_E_CHANGED_MODE => {
            // COM already initialized on this thread in a different model; do not uninit.
            tracing::trace!("UIA probe: COM already initialized with different threading model");
            false
        }
        Err(e) => {
            tracing::trace!(error = ?e, "UIA probe COM initialization failed");
            return SelectionProbe::Unsupported;
        }
    };
    let _com_guard = CoUninitGuard(should_uninit);

    let automation: IUIAutomation =
        match unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) } {
            Ok(v) => v,
            Err(err) => {
                tracing::trace!(error = ?err, "UIA probe automation creation failed");
                return SelectionProbe::Unsupported;
            }
        };

    let focused = match unsafe { automation.GetFocusedElement() } {
        Ok(v) => v,
        Err(err) => {
            tracing::trace!(error = ?err, "UIA probe focused element unavailable");
            return SelectionProbe::Unsupported;
        }
    };

    let pattern: IUIAutomationTextPattern =
        match unsafe { focused.GetCurrentPatternAs(UIA_TextPatternId) } {
            Ok(v) => v,
            Err(err) => {
                tracing::trace!(error = ?err, "UIA text pattern is unsupported");
                return SelectionProbe::Unsupported;
            }
        };

    let selection = match unsafe { pattern.GetSelection() } {
        Ok(v) => v,
        Err(err) => {
            tracing::trace!(error = ?err, "UIA GetSelection failed");
            return SelectionProbe::Unsupported;
        }
    };

    let len = match unsafe { selection.Length() } {
        Ok(v) => v,
        Err(err) => {
            tracing::trace!(error = ?err, "UIA selection length query failed");
            return SelectionProbe::Unsupported;
        }
    };

    if len == 0 {
        return SelectionProbe::NoSelection;
    }

    let range = match unsafe { selection.GetElement(0) } {
        Ok(v) => v,
        Err(err) => return SelectionProbe::SelectionPresentButUnreadable(err),
    };

    let text_bstr = match unsafe { range.GetText((max_chars + 1) as i32) } {
        Ok(v) => v,
        Err(err) => return SelectionProbe::SelectionPresentButUnreadable(err),
    };

    let text = text_bstr.to_string();
    if text.is_empty() {
        return SelectionProbe::NoSelection;
    }

    if !super::convert::is_convertible_selection(&text, max_chars) {
        return SelectionProbe::SelectionPresentButIneligible;
    }

    SelectionProbe::SelectionText(text)
}

/// Attempts to read the current selection from the focused Win32 Edit/RichEdit control.
///
/// This probe never sends synthetic input.
pub(super) fn probe_selection_win32(max_chars: usize) -> SelectionProbe {
    let Some(focus) = focused_hwnd() else {
        return SelectionProbe::Unsupported;
    };

    if !is_supported_edit_control(focus) {
        return SelectionProbe::Unsupported;
    }

    let mut start: u32 = 0;
    let mut end: u32 = 0;
    unsafe {
        let _ = send_message_w(
            focus,
            EM_GETSEL,
            WPARAM((&mut start as *mut u32) as usize),
            LPARAM((&mut end as *mut u32) as isize),
        );
    }

    if start == end {
        return SelectionProbe::NoSelection;
    }

    // Defensive bound: some controls may report indices that do not reflect actual text.
    if (end - start) as usize > max_chars.saturating_mul(2) {
        return SelectionProbe::NoSelection;
    }

    let text = match read_window_text(focus) {
        Some(v) => v,
        None => {
            return SelectionProbe::SelectionPresentButUnreadable(
                windows::core::Error::from_thread(),
            );
        }
    };

    let start_idx = start as usize;
    let end_idx = end as usize;
    if end_idx > text.len() || start_idx > end_idx {
        return SelectionProbe::SelectionPresentButUnreadable(windows::core::Error::from_thread());
    }

    let selection = String::from_utf16_lossy(&text[start_idx..end_idx]);
    if !super::convert::is_convertible_selection(&selection, max_chars) {
        return SelectionProbe::SelectionPresentButIneligible;
    }

    SelectionProbe::SelectionText(selection)
}

fn focused_hwnd() -> Option<HWND> {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return None;
    }

    let tid = unsafe { GetWindowThreadProcessId(fg, None) };
    if tid == 0 {
        return None;
    }

    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };

    let ok = unsafe { GetGUIThreadInfo(tid, &mut info) }.is_ok();
    if !ok || info.hwndFocus.0.is_null() {
        return None;
    }

    Some(info.hwndFocus)
}

fn is_supported_edit_control(hwnd: HWND) -> bool {
    let mut class_name = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
    if len <= 0 {
        return false;
    }

    let class = String::from_utf16_lossy(&class_name[..len as usize]);
    class == "Edit" || class.starts_with("RichEdit") || class.starts_with("RICHEDIT")
}

fn read_window_text(hwnd: HWND) -> Option<Vec<u16>> {
    let len = unsafe { send_message_w(hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 };
    if len <= 0 {
        return Some(Vec::new());
    }

    let cap = (len as usize).saturating_add(1);
    let mut buf = vec![0u16; cap];
    let copied = unsafe {
        send_message_w(
            hwnd,
            WM_GETTEXT,
            WPARAM(cap),
            LPARAM(PWSTR(buf.as_mut_ptr()).0 as isize),
        )
        .0
    };

    if copied < 0 {
        return None;
    }

    buf.truncate(copied as usize);
    Some(buf)
}

unsafe fn send_message_w(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { SendMessageW(hwnd, msg, Some(wparam), Some(lparam)) }
}
