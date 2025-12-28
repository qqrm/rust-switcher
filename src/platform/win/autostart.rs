use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use windows::{
    Win32::{
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize, IPersistFile, STGM, STGM_READ,
        },
        UI::Shell::{
            FOLDERID_Startup, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink,
        },
    },
    core::{Interface, PCWSTR},
};

const SHORTCUT_MARKER: &str = "RustSwitcher Autostart Shortcut";
const SHORTCUT_FILE_NAME: &str = "RustSwitcher.lnk";

pub fn apply_startup_shortcut(enabled: bool) -> windows::core::Result<()> {
    let _com = ComApartment::init()?;

    let startup_dir = startup_folder_path()?;

    cleanup_marked_shortcuts(&startup_dir)?;

    if !enabled {
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| {
        windows::core::Error::new(
            windows::core::HRESULT(0x8000_4005_u32.cast_signed()),
            e.to_string(),
        )
    })?;
    let exe = canonicalize_best_effort(&exe);

    create_shortcut(&startup_dir.join(SHORTCUT_FILE_NAME), &exe)?;

    Ok(())
}

fn canonicalize_best_effort(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    fn init() -> windows::core::Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self { initialized: true })
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

fn startup_folder_path() -> windows::core::Result<PathBuf> {
    let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Startup, KF_FLAG_DEFAULT, None)? };
    let s = pwstr_to_string(raw);

    unsafe {
        CoTaskMemFree(Some(raw.0 as _));
    }

    Ok(PathBuf::from(s))
}

fn pwstr_to_string(p: windows::core::PWSTR) -> String {
    unsafe {
        let mut len = 0usize;
        while *p.0.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(p.0, len);
        String::from_utf16_lossy(slice)
    }
}

fn cleanup_marked_shortcuts(dir: &Path) -> windows::core::Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        windows::core::Error::new(
            windows::core::HRESULT(0x8000_4005_u32.cast_signed()),
            format!("Failed to read startup dir: {e}"),
        )
    })?;

    for ent in entries {
        let Ok(ent) = ent else { continue };
        let path = ent.path();
        if !is_lnk(&path) {
            continue;
        }

        if shortcut_has_marker(&path)? {
            let _ = std::fs::remove_file(&path);
        }
    }

    Ok(())
}

fn is_lnk(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
}

fn shortcut_has_marker(path: &Path) -> windows::core::Result<bool> {
    let shell_link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)? };

    let persist: IPersistFile = shell_link.cast()?;

    let wide = to_wide(path.as_os_str());
    unsafe { persist.Load(PCWSTR(wide.as_ptr()), STGM(STGM_READ.0))? };

    let mut buf = [0u16; 512];
    unsafe {
        shell_link.GetDescription(&mut buf)?;
    }

    Ok(wide_to_string(&buf) == SHORTCUT_MARKER)
}

fn create_shortcut(link_path: &Path, exe_path: &Path) -> windows::core::Result<()> {
    let shell_link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)? };

    let exe_w = to_wide(exe_path.as_os_str());
    unsafe { shell_link.SetPath(PCWSTR(exe_w.as_ptr()))? };

    if let Some(dir) = exe_path.parent() {
        let dir_w = to_wide(dir.as_os_str());
        unsafe { shell_link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr()))? };
    }

    let desc_w = to_wide(OsStr::new(SHORTCUT_MARKER));
    unsafe { shell_link.SetDescription(PCWSTR(desc_w.as_ptr()))? };

    let icon_w = to_wide(exe_path.as_os_str());
    unsafe { shell_link.SetIconLocation(PCWSTR(icon_w.as_ptr()), 0)? };

    let persist: IPersistFile = shell_link.cast()?;

    let link_w = to_wide(link_path.as_os_str());
    unsafe { persist.Save(PCWSTR(link_w.as_ptr()), true)? };

    Ok(())
}

fn to_wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}
