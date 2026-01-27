use windows::{
    Win32::{
        Foundation::{ERROR_HOTKEY_NOT_REGISTERED, HWND},
        UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey},
    },
    core::HRESULT,
};

use crate::config;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    ConvertLastWord,
    PauseToggle,
    ConvertSelection,
    SwitchLayout,
}

// Диапазон 20000+ чтобы не пересекаться с control ids в WM_COMMAND
const HK_ID_BASE: i32 = 20000;

pub const HK_CONVERT_LAST_WORD_ID: i32 = HK_ID_BASE + 1;
pub const HK_PAUSE_TOGGLE_ID: i32 = HK_ID_BASE + 2;
pub const HK_CONVERT_SELECTION_ID: i32 = HK_ID_BASE + 3;
pub const HK_SWITCH_LAYOUT_ID: i32 = HK_ID_BASE + 4;

pub fn action_from_id(id: i32) -> Option<HotkeyAction> {
    match id {
        HK_CONVERT_LAST_WORD_ID => Some(HotkeyAction::ConvertLastWord),
        HK_PAUSE_TOGGLE_ID => Some(HotkeyAction::PauseToggle),
        HK_CONVERT_SELECTION_ID => Some(HotkeyAction::ConvertSelection),
        HK_SWITCH_LAYOUT_ID => Some(HotkeyAction::SwitchLayout),
        _ => None,
    }
}

fn unregister_one_quiet(hwnd: HWND, id: i32) -> windows::core::Result<()> {
    if let Err(e) = unsafe { UnregisterHotKey(Some(hwnd), id) }
        && e.code() != HRESULT::from_win32(ERROR_HOTKEY_NOT_REGISTERED.0)
    {
        return Err(e);
    }
    Ok(())
}

pub fn unregister_all(hwnd: HWND) -> windows::core::Result<()> {
    for id in [
        HK_CONVERT_LAST_WORD_ID,
        HK_PAUSE_TOGGLE_ID,
        HK_CONVERT_SELECTION_ID,
        HK_SWITCH_LAYOUT_ID,
    ] {
        unregister_one_quiet(hwnd, id)?;
    }

    Ok(())
}

fn register_one(hwnd: HWND, id: i32, hk: Option<config::Hotkey>) -> windows::core::Result<()> {
    let Some(hk) = hk else {
        #[cfg(debug_assertions)]
        crate::utils::helpers::debug_log(&format!("hotkey id={id} disabled"));
        return Ok(());
    };

    if hk.vk == 0 {
        #[cfg(debug_assertions)]
        crate::utils::helpers::debug_log(&format!(
            "hotkey id={} ignored: invalid vk=0x0 mods=0x{:X}",
            id, hk.mods
        ));
        return Ok(());
    }

    #[cfg(debug_assertions)]
    crate::utils::helpers::debug_log(&format!(
        "RegisterHotKey id={} mods=0x{:X} vk=0x{:X}",
        id, hk.mods, hk.vk
    ));

    unsafe {
        RegisterHotKey(Some(hwnd), id, HOT_KEY_MODIFIERS(hk.mods), hk.vk)?;
    }

    #[cfg(debug_assertions)]
    crate::utils::helpers::debug_log(&format!("RegisterHotKey OK id={id}"));

    Ok(())
}

pub fn register_from_config(hwnd: HWND, cfg: &config::Config) -> windows::core::Result<()> {
    unregister_all(hwnd)?;

    register_one(
        hwnd,
        HK_CONVERT_LAST_WORD_ID,
        cfg.hotkey_convert_last_word(),
    )?;
    register_one(hwnd, HK_PAUSE_TOGGLE_ID, cfg.hotkey_pause())?;
    register_one(
        hwnd,
        HK_CONVERT_SELECTION_ID,
        cfg.hotkey_convert_selection(),
    )?;
    register_one(hwnd, HK_SWITCH_LAYOUT_ID, cfg.hotkey_switch_layout())?;

    Ok(())
}
