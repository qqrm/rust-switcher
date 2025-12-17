use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotkey {
    pub vk: u32,
    pub mods: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub start_on_startup: bool,
    pub show_tray_icon: bool,
    pub delay_ms: u32,
    pub hotkey_convert_last_word: Option<Hotkey>,
    pub hotkey_convert_selection: Option<Hotkey>,
    pub hotkey_switch_layout: Option<Hotkey>,
    pub hotkey_pause: Option<Hotkey>,
    pub paused: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            start_on_startup: false,
            show_tray_icon: false,
            delay_ms: 100,
            hotkey_convert_last_word: Some(Hotkey { vk: 0x13, mods: 0 }), // VK_PAUSE
            hotkey_convert_selection: Some(Hotkey {
                vk: 0x03,
                mods: 0x0002,
            }), // VK_CANCEL + MOD_CONTROL
            hotkey_switch_layout: None,
            hotkey_pause: None,
            paused: false,
        }
    }
}

pub fn config_path() -> io::Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA is not set"))?;
    Ok(PathBuf::from(appdata)
        .join("RustSwitcher")
        .join("config.json"))
}

fn ensure_config_dir(path: &PathBuf) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

pub fn load() -> io::Result<Config> {
    let path = config_path()?;
    ensure_config_dir(&path)?;
    confy::load_path(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

pub fn save(cfg: &Config) -> io::Result<()> {
    let path = config_path()?;
    ensure_config_dir(&path)?;
    confy::store_path(path, cfg).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
