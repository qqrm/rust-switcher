use std::{
    mem,
    path::{Path, PathBuf},
};

use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR},
        System::{
            Com::CoTaskMemFree,
            Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey,
                RegCreateKeyW,
                RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
            },
        },
        UI::Shell::{FOLDERID_Startup, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    },
    core::{Error, HRESULT, PCWSTR},
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "RustSwitcher";
const LEGACY_SHORTCUT_FILE_NAME: &str = "RustSwitcher Autostart.lnk";

/// Returns whether the current executable is registered to start with Windows.
pub fn is_enabled() -> windows::core::Result<bool> {
    let Some((value_type, bytes)) = read_value(RUN_KEY, VALUE_NAME)? else {
        return Ok(false);
    };
    let Some(registered) = decode_registry_string(value_type, &bytes) else {
        return Ok(false);
    };

    Ok(registered.eq_ignore_ascii_case(&current_command()?))
}

/// Migrates the previous Startup-folder shortcut to the per-user Run registry key.
///
/// This preserves an existing enabled setting the first time a version using the
/// registry-based implementation starts.
pub fn migrate_legacy_shortcut() -> windows::core::Result<()> {
    let legacy_path = legacy_shortcut_path()?;
    if is_enabled()? {
        remove_legacy_shortcut(&legacy_path)?;
        return Ok(());
    }

    if legacy_path.exists() {
        set_enabled(true)?;
    }

    Ok(())
}

/// Enables or disables per-user autostart for the current executable.
pub fn set_enabled(enabled: bool) -> windows::core::Result<()> {
    let legacy_path = legacy_shortcut_path()?;

    if enabled {
        write_string(RUN_KEY, VALUE_NAME, &current_command()?)?;
        remove_legacy_shortcut(&legacy_path)?;
    } else {
        remove_legacy_shortcut(&legacy_path)?;
        delete_value(RUN_KEY, VALUE_NAME)?;
    }

    Ok(())
}

fn current_command() -> windows::core::Result<String> {
    let executable = std::env::current_exe().map_err(|error| {
        Error::new(
            HRESULT(0x8000_4005_u32.cast_signed()),
            format!("cannot resolve current executable: {error}"),
        )
    })?;
    Ok(command_for_executable(&executable))
}

fn command_for_executable(executable: &Path) -> String {
    format!("\"{}\" {}", executable.display(), super::AUTOSTART_ARG)
}

fn legacy_shortcut_path() -> windows::core::Result<PathBuf> {
    let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Startup, KF_FLAG_DEFAULT, None)? };
    let startup_dir = pwstr_to_path(raw);
    unsafe {
        CoTaskMemFree(Some(raw.0.cast()));
    }
    Ok(startup_dir.join(LEGACY_SHORTCUT_FILE_NAME))
}

fn pwstr_to_path(value: windows::core::PWSTR) -> PathBuf {
    // SAFETY: `value` is a NUL-terminated string allocated by SHGetKnownFolderPath.
    let mut len = 0_usize;
    unsafe {
        while *value.0.add(len) != 0 {
            len += 1;
        }
        let chars = std::slice::from_raw_parts(value.0, len);
        PathBuf::from(String::from_utf16_lossy(chars))
    }
}

fn remove_legacy_shortcut(path: &Path) -> windows::core::Result<()> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error("cannot inspect legacy startup shortcut", error)),
    };

    if metadata.permissions().readonly() {
        let mut permissions = metadata.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions)
            .map_err(|error| io_error("cannot make legacy startup shortcut writable", error))?;
    }

    std::fs::remove_file(path)
        .map_err(|error| io_error("cannot remove legacy startup shortcut", error))
}

fn decode_registry_string(
    value_type: windows::Win32::System::Registry::REG_VALUE_TYPE,
    bytes: &[u8],
) -> Option<String> {
    let (utf16_bytes, remainder) = bytes.as_chunks::<2>();
    if value_type != REG_SZ || !remainder.is_empty() {
        return None;
    }

    let words = utf16_bytes
        .iter()
        .map(|bytes| u16::from_le_bytes(*bytes))
        .take_while(|character| *character != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&words).ok()
}

fn read_value(
    subkey: &str,
    name: &str,
) -> windows::core::Result<
    Option<(
        windows::Win32::System::Registry::REG_VALUE_TYPE,
        Vec<u8>,
    )>,
> {
    let subkey = to_wide(subkey);
    let name = to_wide(name);
    let mut key = HKEY::default();
    // SAFETY: the key path is NUL-terminated and `key` is an output pointer.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_QUERY_VALUE,
            &mut key,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    check_status("RegOpenKeyExW failed", status)?;
    let key = RegistryKey(key);

    let mut value_type = REG_SZ;
    let mut size = 0_u32;
    // SAFETY: `key` is open for querying and output pointers are valid.
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut value_type),
            None,
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    check_status("RegQueryValueExW failed", status)?;

    let mut bytes = vec![0_u8; size as usize];
    // SAFETY: `bytes` has the size reported by the previous query.
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut value_type),
            Some(bytes.as_mut_ptr()),
            Some(&mut size),
        )
    };
    check_status("RegQueryValueExW failed", status)?;
    bytes.truncate(size as usize);

    Ok(Some((value_type, bytes)))
}

fn write_string(subkey: &str, name: &str, value: &str) -> windows::core::Result<()> {
    let words = to_wide(value);
    // SAFETY: `words` is a contiguous UTF-16 buffer and remains alive for the registry call.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            words.as_ptr().cast(),
            words.len() * mem::size_of::<u16>(),
        )
    };
    write_value(subkey, name, REG_SZ, bytes)
}

fn write_value(
    subkey: &str,
    name: &str,
    value_type: windows::Win32::System::Registry::REG_VALUE_TYPE,
    data: &[u8],
) -> windows::core::Result<()> {
    let subkey = to_wide(subkey);
    let name = to_wide(name);
    let mut key = HKEY::default();
    // SAFETY: the key path is NUL-terminated and `key` is an output pointer.
    let status = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), &mut key) };
    check_status("RegCreateKeyW failed", status)?;
    let key = RegistryKey(key);

    // SAFETY: `key` is open for writing and `data` remains valid for the call.
    let status = unsafe {
        RegSetValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            Some(0),
            value_type,
            Some(data),
        )
    };
    check_status("RegSetValueExW failed", status)
}

fn delete_value(subkey: &str, name: &str) -> windows::core::Result<()> {
    let subkey = to_wide(subkey);
    let name = to_wide(name);
    let mut key = HKEY::default();
    // SAFETY: the key path is NUL-terminated and `key` is an output pointer.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut key,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check_status("RegOpenKeyExW failed", status)?;
    let key = RegistryKey(key);

    // SAFETY: `key` is open for writing and the value name is NUL-terminated.
    let status = unsafe { RegDeleteValueW(key.0, PCWSTR(name.as_ptr())) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check_status("RegDeleteValueW failed", status)
}

fn check_status(operation: &str, status: WIN32_ERROR) -> windows::core::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(Error::new(
            HRESULT(0x8000_4005_u32.cast_signed()),
            format!("{operation}: Windows error {}", status.0),
        ))
    }
}

fn io_error(operation: &str, error: std::io::Error) -> Error {
    Error::new(
        HRESULT(0x8000_4005_u32.cast_signed()),
        format!("{operation}: {error}"),
    )
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

struct RegistryKey(HKEY);

impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns the registry key handle.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::{command_for_executable, decode_registry_string};
    use std::path::Path;
    use windows::Win32::System::Registry::{REG_DWORD, REG_SZ};

    #[test]
    fn command_quotes_the_executable_and_starts_hidden() {
        assert_eq!(
            command_for_executable(Path::new(r"C:\Program Files\RustSwitcher\rust-switcher.exe")),
            r#""C:\Program Files\RustSwitcher\rust-switcher.exe" --autostart"#,
        );
    }

    #[test]
    fn decode_registry_string_requires_a_string_value() {
        let bytes = "RustSwitcher\0ignored"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();

        assert_eq!(
            decode_registry_string(REG_SZ, &bytes).as_deref(),
            Some("RustSwitcher"),
        );
        assert!(decode_registry_string(REG_DWORD, &bytes).is_none());
    }
}
