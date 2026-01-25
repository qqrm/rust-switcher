use std::sync::OnceLock;

use windows::{
    Win32::{
        Foundation::HMODULE,
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
    core::{PCSTR, w},
};

#[repr(i32)]
#[derive(Clone, Copy)]
enum PreferredAppMode {
    ForceDark = 2,
    ForceLight = 3,
}

type SetPreferredAppModeFn = unsafe extern "system" fn(PreferredAppMode) -> PreferredAppMode;
type FlushMenuThemesFn = unsafe extern "system" fn();

struct UxThemeFns {
    set_preferred_app_mode: SetPreferredAppModeFn,
    flush_menu_themes: FlushMenuThemesFn,
}

fn get_uxtheme_fns() -> Option<&'static UxThemeFns> {
    static FNS: OnceLock<Option<UxThemeFns>> = OnceLock::new();

    FNS.get_or_init(|| unsafe {
        let uxtheme: HMODULE = LoadLibraryW(w!("uxtheme.dll")).ok()?;

        // Undocumented ordinal exports:
        // 135 = SetPreferredAppMode
        // 136 = FlushMenuThemes
        let set_ptr = GetProcAddress(uxtheme, PCSTR(135usize as *const u8))?;
        let flush_ptr = GetProcAddress(uxtheme, PCSTR(136usize as *const u8))?;

        let set_preferred_app_mode = std::mem::transmute::<
            *const core::ffi::c_void,
            SetPreferredAppModeFn,
        >(set_ptr as *const core::ffi::c_void);

        let flush_menu_themes = std::mem::transmute::<*const core::ffi::c_void, FlushMenuThemesFn>(
            flush_ptr as *const core::ffi::c_void,
        );

        Some(UxThemeFns {
            set_preferred_app_mode,
            flush_menu_themes,
        })
    })
    .as_ref()
}

pub(crate) fn set_tray_menu_preferred_theme(is_dark: bool) {
    let Some(fns) = get_uxtheme_fns() else {
        return;
    };

    let mode = if is_dark {
        PreferredAppMode::ForceDark
    } else {
        PreferredAppMode::ForceLight
    };

    unsafe {
        let _ = (fns.set_preferred_app_mode)(mode);
    }
}

pub(crate) fn flush_tray_menu_theme() {
    let Some(fns) = get_uxtheme_fns() else {
        return;
    };

    unsafe {
        (fns.flush_menu_themes)();
    }
}
