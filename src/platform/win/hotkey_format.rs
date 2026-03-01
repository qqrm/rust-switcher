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
        mods_vks: hk.mods_vks,
        vk: (hk.vk != 0).then_some(hk.vk),
    };

    format_hotkey_chord(chord)
}

pub(crate) fn format_hotkey_sequence(seq: Option<config::HotkeySequence>) -> String {
    let Some(seq) = seq else {
        return "None".to_string();
    };

    std::iter::once(seq.first)
        .chain(seq.second)
        .map(format_hotkey_chord)
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_hotkey_chord(ch: config::HotkeyChord) -> String {
    fn vk_to_display(vk: u32) -> String {
        if (0x41..=0x5A).contains(&vk) || (0x30..=0x39).contains(&vk) {
            // Safe due to explicit ASCII range checks above.
            return (vk as u8 as char).to_string();
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
            return format!("VK 0x{vk:02X}");
        };
        String::from_utf16_lossy(&buf[..len])
    }

    const MODS_VKS_ORDER: &[(u32, &str)] = &[
        (config::MODVK_LCTRL, "LCtrl"),
        (config::MODVK_RCTRL, "RCtrl"),
        (config::MODVK_LALT, "LAlt"),
        (config::MODVK_RALT, "RAlt"),
        (config::MODVK_LSHIFT, "LShift"),
        (config::MODVK_RSHIFT, "RShift"),
        (config::MODVK_LWIN, "LWin"),
        (config::MODVK_RWIN, "RWin"),
    ];
    const MODS_ORDER: &[(u32, &str)] = &[
        (MOD_CONTROL.0, "Ctrl"),
        (MOD_ALT.0, "Alt"),
        (MOD_SHIFT.0, "Shift"),
        (MOD_WIN.0, "Win"),
    ];

    let mut mods: Vec<&'static str> = Vec::new();

    if ch.mods_vks != 0 {
        mods.extend(
            MODS_VKS_ORDER
                .iter()
                .filter_map(|(mask, label)| ((ch.mods_vks & mask) != 0).then_some(*label)),
        );
    } else {
        mods.extend(
            MODS_ORDER
                .iter()
                .filter_map(|(mask, label)| ((ch.mods & mask) != 0).then_some(*label)),
        );
    }

    let mut out = mods.join(" + ");

    if let Some(vk) = ch.vk {
        let key = vk_to_display(vk);
        if !out.is_empty() {
            out.push_str(" + ");
        }
        out.push_str(&key);
    }

    if out.is_empty() {
        "None".to_string()
    } else {
        out
    }
}
