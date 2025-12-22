use windows::Win32::UI::{
    Input::KeyboardAndMouse::{
        MAPVK_VSC_TO_VK_EX, MapVirtualKeyW, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_MENU,
        VK_RCONTROL, VK_RMENU, VK_SHIFT,
    },
    WindowsAndMessaging::{
        KBDLLHOOKSTRUCT, LLKHF_EXTENDED, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    },
};

use crate::config;

pub fn is_keydown_msg(msg: u32) -> bool {
    msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN
}

pub fn is_keyup_msg(msg: u32) -> bool {
    msg == WM_KEYUP || msg == WM_SYSKEYUP
}

pub fn normalize_vk(kb: &KBDLLHOOKSTRUCT) -> u32 {
    let vk = kb.vkCode;
    let extended = kb.flags.contains(LLKHF_EXTENDED);

    match vk {
        x if x == VK_SHIFT.0 as u32 => {
            let mapped = unsafe { MapVirtualKeyW(kb.scanCode, MAPVK_VSC_TO_VK_EX) };
            if mapped != 0 { mapped } else { vk }
        }
        x if x == VK_CONTROL.0 as u32 => {
            if extended {
                VK_RCONTROL.0 as u32
            } else {
                VK_LCONTROL.0 as u32
            }
        }
        x if x == VK_MENU.0 as u32 => {
            if extended {
                VK_RMENU.0 as u32
            } else {
                VK_LMENU.0 as u32
            }
        }
        _ => vk,
    }
}

pub fn mod_bit_for_vk(vk: u32) -> Option<u32> {
    match vk {
        0xA2 | 0xA3 => Some(windows::Win32::UI::Input::KeyboardAndMouse::MOD_CONTROL.0),
        0xA0 | 0xA1 => Some(windows::Win32::UI::Input::KeyboardAndMouse::MOD_SHIFT.0),
        0xA4 | 0xA5 => Some(windows::Win32::UI::Input::KeyboardAndMouse::MOD_ALT.0),
        0x5B | 0x5C => Some(windows::Win32::UI::Input::KeyboardAndMouse::MOD_WIN.0),
        _ => None,
    }
}

pub fn mod_vk_bit_for_vk(vk: u32) -> Option<u32> {
    match vk {
        0xA2 => Some(config::MODVK_LCTRL),
        0xA3 => Some(config::MODVK_RCTRL),
        0xA0 => Some(config::MODVK_LSHIFT),
        0xA1 => Some(config::MODVK_RSHIFT),
        0xA4 => Some(config::MODVK_LALT),
        0xA5 => Some(config::MODVK_RALT),
        0x5B => Some(config::MODVK_LWIN),
        0x5C => Some(config::MODVK_RWIN),
        _ => None,
    }
}
