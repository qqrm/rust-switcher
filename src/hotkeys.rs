use crate::config;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    ConvertLastWord,
    PauseToggle,
    ConvertSelection,
    SwitchLayout,
}

// Диапазон 1000+ чтобы не пересекаться с control ids в WM_COMMAND
pub const HK_CONVERT_LAST_WORD_ID: i32 = 1001;
pub const HK_PAUSE_TOGGLE_ID: i32 = 1002;
pub const HK_CONVERT_SELECTION_ID: i32 = 1003;
pub const HK_SWITCH_LAYOUT_ID: i32 = 1004;

pub fn action_from_id(id: i32) -> Option<HotkeyAction> {
    match id {
        HK_CONVERT_LAST_WORD_ID => Some(HotkeyAction::ConvertLastWord),
        HK_PAUSE_TOGGLE_ID => Some(HotkeyAction::PauseToggle),
        HK_CONVERT_SELECTION_ID => Some(HotkeyAction::ConvertSelection),
        HK_SWITCH_LAYOUT_ID => Some(HotkeyAction::SwitchLayout),
        _ => None,
    }
}

pub fn unregister_all(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), HK_CONVERT_LAST_WORD_ID);
        let _ = UnregisterHotKey(Some(hwnd), HK_PAUSE_TOGGLE_ID);
        let _ = UnregisterHotKey(Some(hwnd), HK_CONVERT_SELECTION_ID);
        let _ = UnregisterHotKey(Some(hwnd), HK_SWITCH_LAYOUT_ID);
    }
}

fn register_one(hwnd: HWND, id: i32, hk: Option<config::Hotkey>) -> windows::core::Result<()> {
    let Some(hk) = hk else {
        return Ok(());
    };

    unsafe {
        let _ = RegisterHotKey(Some(hwnd), id, HOT_KEY_MODIFIERS(hk.mods), hk.vk)?;
    }

    Ok(())
}

pub fn register_from_config(hwnd: HWND, cfg: &config::Config) -> windows::core::Result<()> {
    // Перерегистрация безопаснее так: сначала снять все, потом поставить по конфигу
    unregister_all(hwnd);

    register_one(hwnd, HK_CONVERT_LAST_WORD_ID, cfg.hotkey_convert_last_word)?;
    register_one(hwnd, HK_PAUSE_TOGGLE_ID, cfg.hotkey_pause)?;
    register_one(hwnd, HK_CONVERT_SELECTION_ID, cfg.hotkey_convert_selection)?;
    register_one(hwnd, HK_SWITCH_LAYOUT_ID, cfg.hotkey_switch_layout)?;

    Ok(())
}
