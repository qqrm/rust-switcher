use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyNameTextW, MAPVK_VK_TO_VSC, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, MapVirtualKeyW,
};

use crate::config;

pub(crate) fn format_hotkey(hk: Option<config::Hotkey>) -> String {
    let Some(hk) = hk else {
        return "None".to_string();
    };

    let chord = config::HotkeyChord {
        mods: hk.mods,
        mods_vks: 0,
        vk: (hk.vk != 0).then_some(hk.vk),
    };

    format_hotkey_chord(chord)
}

pub(crate) fn format_hotkey_sequence(seq: Option<config::HotkeySequence>) -> String {
    let Some(seq) = seq else {
        return "None".to_string();
    };

    let mut chords: Vec<String> = Vec::new();
    chords.push(format_hotkey_chord(seq.first));
    if let Some(c1) = seq.second {
        chords.push(format_hotkey_chord(c1));
    }

    chords.join("; ")
}

fn format_hotkey_chord(ch: config::HotkeyChord) -> String {
    fn vk_to_display(vk: u32) -> String {
        if (0x41..=0x5A).contains(&vk) || (0x30..=0x39).contains(&vk) {
            let Ok(vk_u8) = u8::try_from(vk) else {
                return vk.to_string();
            };
            return (vk_u8 as char).to_string();
        }

        let sc = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) };
        if sc == 0 {
            return format!("VK 0x{vk:02X}");
        }

        let lparam = (sc.cast_signed() << 16) as i32;

        let mut buf = [0u16; 64];
        let len = unsafe { GetKeyNameTextW(lparam, &mut buf) };
        if len <= 0 {
            return format!("VK 0x{vk:02X}");
        }

        let Ok(len) = usize::try_from(len) else {
            return String::new(); // или return vk.to_string(), если это функция форматирования
        };
        String::from_utf16_lossy(&buf[..len])
    }

    let mut parts: Vec<String> = Vec::new();

    if ch.mods_vks != 0 {
        if (ch.mods_vks & config::MODVK_LCTRL) != 0 {
            parts.push("LCtrl".to_string());
        }
        if (ch.mods_vks & config::MODVK_RCTRL) != 0 {
            parts.push("RCtrl".to_string());
        }
        if (ch.mods_vks & config::MODVK_LALT) != 0 {
            parts.push("LAlt".to_string());
        }
        if (ch.mods_vks & config::MODVK_RALT) != 0 {
            parts.push("RAlt".to_string());
        }
        if (ch.mods_vks & config::MODVK_LSHIFT) != 0 {
            parts.push("LShift".to_string());
        }
        if (ch.mods_vks & config::MODVK_RSHIFT) != 0 {
            parts.push("RShift".to_string());
        }
        if (ch.mods_vks & config::MODVK_LWIN) != 0 {
            parts.push("LWin".to_string());
        }
        if (ch.mods_vks & config::MODVK_RWIN) != 0 {
            parts.push("RWin".to_string());
        }
    } else {
        if (ch.mods & MOD_CONTROL.0) != 0 {
            parts.push("Ctrl".to_string());
        }
        if (ch.mods & MOD_ALT.0) != 0 {
            parts.push("Alt".to_string());
        }
        if (ch.mods & MOD_SHIFT.0) != 0 {
            parts.push("Shift".to_string());
        }
        if (ch.mods & MOD_WIN.0) != 0 {
            parts.push("Win".to_string());
        }
    }

    if let Some(vk) = ch.vk {
        parts.push(vk_to_display(vk));
    }

    if parts.is_empty() {
        "None".to_string()
    } else {
        parts.join(" + ")
    }
}
