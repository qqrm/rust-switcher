#![cfg(windows)]

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use super::env_lock::lock_env;
use crate::config;

fn restore_appdata(old: Option<std::ffi::OsString>) {
    match old {
        Some(v) => unsafe { std::env::set_var("APPDATA", v) },
        None => unsafe { std::env::remove_var("APPDATA") },
    }
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rust-switcher-tests-{prefix}-{ts}"))
}

#[test]
fn config_path_errors_when_appdata_missing() {
    let _g = lock_env();

    let old = std::env::var_os("APPDATA");
    unsafe { std::env::remove_var("APPDATA") };

    let err = config::config_path().unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert!(err.to_string().contains("APPDATA"));

    restore_appdata(old);
}

#[test]
fn config_path_uses_app_dir_and_filename() {
    let _g = lock_env();

    let old = std::env::var_os("APPDATA");
    let dir = unique_temp_dir("appdata-path");
    fs::create_dir_all(&dir).unwrap();
    unsafe { std::env::set_var("APPDATA", &dir) };

    let p = config::config_path().unwrap();
    let s = p.to_string_lossy().to_string();

    assert!(s.contains(&*dir.to_string_lossy()));
    assert!(s.ends_with(r"\RustSwitcher\config.json"));

    restore_appdata(old);
    let _ = fs::remove_dir_all(dir);
}
