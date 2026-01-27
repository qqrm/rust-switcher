use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use windows::Win32::UI::Input::KeyboardAndMouse::MOD_CONTROL;

use crate::config::{self, Config, HotkeyChord, HotkeySequence, RawConfig};

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

struct AppDataOverride {
    _guard: std::sync::MutexGuard<'static, ()>,
    old: Option<std::ffi::OsString>,
    dir: PathBuf,
}

impl AppDataOverride {
    fn new(prefix: &str) -> Self {
        let guard = lock_env();

        let old = std::env::var_os("APPDATA");
        let dir = unique_temp_dir(prefix);
        fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("APPDATA", &dir) };

        Self {
            _guard: guard,
            old,
            dir,
        }
    }
}

impl Drop for AppDataOverride {
    fn drop(&mut self) {
        match self.old.take() {
            Some(v) => unsafe { std::env::set_var("APPDATA", v) },
            None => unsafe { std::env::remove_var("APPDATA") },
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn config_save_and_load_roundtrip_via_appdata() {
    let _env = AppDataOverride::new("appdata");

    let raw_config = RawConfig {
        hotkey_pause_sequence: Some(seq_ctrl_a()),
        ..Default::default()
    };

    let config = Config::try_from(raw_config).unwrap();
    config::save(&config).unwrap();
    let loaded = config::load().unwrap();

    assert_eq!(
        loaded.hotkey_pause_sequence(),
        config.hotkey_pause_sequence()
    );
}

#[test]
fn config_save_rejects_invalid_sequences() {
    let raw_config = RawConfig {
        hotkey_convert_last_word_sequence: Some(seq_ctrl_a()),
        hotkey_pause_sequence: Some(seq_ctrl_a()),
        ..Default::default()
    };

    let err = Config::try_from(raw_config).unwrap_err();
    assert!(err.contains("unique hotkey sequence"));
}
