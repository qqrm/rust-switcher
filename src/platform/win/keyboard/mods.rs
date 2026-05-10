use std::sync::atomic::{AtomicU32, Ordering};

use crate::{
    config::{self, MODVK_LCTRL, MODVK_RALT},
    platform::win::keyboard::vk::{mod_bit_for_vk, mod_vk_bit_for_vk},
};

static MODS_DOWN: AtomicU32 = AtomicU32::new(0);
static MODVKS_DOWN: AtomicU32 = AtomicU32::new(0);

pub(crate) fn update_mods_down_press(vk: u32) {
    if let Some(bit) = mod_bit_for_vk(vk) {
        MODS_DOWN.fetch_or(bit, Ordering::Relaxed);
    }
    if let Some(bit) = mod_vk_bit_for_vk(vk) {
        MODVKS_DOWN.fetch_or(bit, Ordering::Relaxed);
    }
}

pub(crate) fn update_mods_down_release(vk: u32) {
    if let Some(bit) = mod_bit_for_vk(vk) {
        MODS_DOWN.fetch_and(!bit, Ordering::Relaxed);
    }
    if let Some(bit) = mod_vk_bit_for_vk(vk) {
        MODVKS_DOWN.fetch_and(!bit, Ordering::Relaxed);
    }
}

pub(crate) fn chord_from_vk(vk: u32) -> config::HotkeyChord {
    let mods = MODS_DOWN.load(Ordering::Relaxed);
    let mut mods_vks = MODVKS_DOWN.load(Ordering::Relaxed);

    // Windows AltGr often arrives as RAlt + LCtrl
    if (mods_vks & MODVK_RALT) != 0 {
        mods_vks &= !MODVK_LCTRL;
    }

    config::HotkeyChord {
        mods,
        mods_vks,
        vk: Some(vk),
    }
}

pub(crate) fn mods_now() -> u32 {
    MODS_DOWN.load(Ordering::Relaxed)
}

#[cfg(test)]
pub(crate) fn reset_mods_state() {
    MODS_DOWN.store(0, Ordering::Relaxed);
    MODVKS_DOWN.store(0, Ordering::Relaxed);
}
