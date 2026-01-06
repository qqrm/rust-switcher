use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use windows::Win32::UI::Input::KeyboardAndMouse::MOD_CONTROL;

use crate::config::{self, Config, HotkeyChord, HotkeySequence};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rust-switcher-tests-{prefix}-{ts}"))
}

fn seq_ctrl_a() -> HotkeySequence {
    HotkeySequence {
        first: HotkeyChord {
            mods: MOD_CONTROL.0,
            mods_vks: 0,
            vk: Some(u32::from(b'A')),
        },
        second: None,
        max_gap_ms: 1000,
    }
}

fn restore_appdata(old: Option<std::ffi::OsString>) {
    match old {
        Some(v) => unsafe { std::env::set_var("APPDATA", v) },
        None => unsafe { std::env::remove_var("APPDATA") },
    }
}

#[test]
fn config_save_and_load_roundtrip_via_appdata() {
    let _g = lock_env();

    let old = std::env::var_os("APPDATA");
    let dir = unique_temp_dir("appdata");
    fs::create_dir_all(&dir).unwrap();
    unsafe { std::env::set_var("APPDATA", &dir) };

    let cfg = Config {
        hotkey_pause_sequence: Some(seq_ctrl_a()),
        ..Default::default()
    };

    config::save(&cfg).unwrap();
    let loaded = config::load().unwrap();

    assert_eq!(loaded.hotkey_pause_sequence, cfg.hotkey_pause_sequence);

    restore_appdata(old);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn config_save_rejects_invalid_sequences() {
    let _g = lock_env();

    let old = std::env::var_os("APPDATA");
    let dir = unique_temp_dir("appdata-invalid");
    fs::create_dir_all(&dir).unwrap();
    unsafe { std::env::set_var("APPDATA", &dir) };

    let cfg = Config {
        hotkey_convert_last_word_sequence: Some(seq_ctrl_a()),
        hotkey_pause_sequence: Some(seq_ctrl_a()),
        ..Default::default()
    };

    let err = config::save(&cfg).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("unique hotkey sequence"));

    restore_appdata(old);
    let _ = fs::remove_dir_all(dir);
}
