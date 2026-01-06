use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use windows::{
    Win32::{
        Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize, IPersistFile,
        },
        UI::Shell::{
            FOLDERID_Startup, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink,
        },
    },
    core::{Interface, PCWSTR},
};

const SHORTCUT_MARKER: &str = "RustSwitcher Autostart Shortcut";
const SHORTCUT_FILE_NAME: &str = "RustSwitcher Autostart.lnk";

pub fn is_enabled() -> windows::core::Result<bool> {
    let _com = ComApartment::init()?;
    let startup_dir = startup_folder_path()?;
    let path = startup_dir.join(SHORTCUT_FILE_NAME);

    match std::fs::metadata(&path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(windows::core::Error::new(
            windows::core::HRESULT(0x8000_4005_u32.cast_signed()),
            format!("Failed to stat startup shortcut {:?}: {e}", path),
        )),
    }
}

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

    create_shortcut(&startup_dir.join(SHORTCUT_FILE_NAME), &exe)?;

    Ok(())
}

struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    fn init() -> windows::core::Result<Self> {
        unsafe {
            match CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() {
                Ok(()) => Ok(Self { initialized: true }),
                Err(e) if e.code() == windows::core::HRESULT(0x8001_0106_u32.cast_signed()) => {
                    Ok(Self { initialized: false })
                }
                Err(e) => Err(e),
            }
        }
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
    force_remove_file_or_error(&dir.join(SHORTCUT_FILE_NAME))
}

fn force_remove_file_or_error(path: &Path) -> windows::core::Result<()> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x8000_4005_u32.cast_signed()),
                format!("Failed to stat file {:?}: {e}", path),
            ));
        }
    };

    if meta.permissions().readonly() {
        let mut perm = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = std::fs::set_permissions(path, perm);
    }

    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = schedule_delete_on_reboot(path);
            Err(windows::core::Error::new(
                windows::core::HRESULT(0x8000_4005_u32.cast_signed()),
                format!("Failed to remove startup shortcut {:?}: {e}", path),
            ))
        }
    }
}

fn schedule_delete_on_reboot(path: &Path) -> windows::core::Result<()> {
    let w = to_wide(path.as_os_str());
    unsafe {
        MoveFileExW(
            PCWSTR(w.as_ptr()),
            PCWSTR::null(),
            MOVE_FILE_FLAGS(0x0000_0004),
        )?;
    }
    Ok(())
}

fn with_ctx<T>(r: windows::core::Result<T>, what: &'static str) -> windows::core::Result<T> {
    r.map_err(|e| windows::core::Error::new(e.code(), format!("{what}: {e}")))
}

fn create_shortcut(link_path: &Path, exe_path: &Path) -> windows::core::Result<()> {
    force_remove_file_or_error(link_path)?;

    let shell_link: IShellLinkW = with_ctx(
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) },
        "CoCreateInstance(ShellLink)",
    )?;

    let exe_w = to_wide(exe_path.as_os_str());
    with_ctx(
        unsafe { shell_link.SetPath(PCWSTR(exe_w.as_ptr())) },
        "IShellLinkW::SetPath",
    )?;

    let args_w = to_wide(OsStr::new(super::AUTOSTART_ARG));
    with_ctx(
        unsafe { shell_link.SetArguments(PCWSTR(args_w.as_ptr())) },
        "IShellLinkW::SetArguments",
    )?;

    if let Some(dir) = exe_path.parent() {
        let dir_w = to_wide(dir.as_os_str());
        with_ctx(
            unsafe { shell_link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr())) },
            "IShellLinkW::SetWorkingDirectory",
        )?;
    }

    let desc_w = to_wide(OsStr::new(SHORTCUT_MARKER));
    with_ctx(
        unsafe { shell_link.SetDescription(PCWSTR(desc_w.as_ptr())) },
        "IShellLinkW::SetDescription",
    )?;

    let icon_w = to_wide(exe_path.as_os_str());
    with_ctx(
        unsafe { shell_link.SetIconLocation(PCWSTR(icon_w.as_ptr()), 0) },
        "IShellLinkW::SetIconLocation",
    )?;

    let persist: IPersistFile = with_ctx(shell_link.cast(), "QueryInterface(IPersistFile)")?;

    let link_w = to_wide(link_path.as_os_str());
    with_ctx(
        unsafe { persist.Save(PCWSTR(link_w.as_ptr()), true) },
        "IPersistFile::Save",
    )?;

    Ok(())
}

fn to_wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}
