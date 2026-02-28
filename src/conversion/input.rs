use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL,
};

/// Virtual key code for the Left Arrow key.
///
/// Used to move the caret left when selecting previously inserted text.
const VK_LEFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x25);

/// Virtual key code for the Right Arrow key.
///
/// Used to move the caret right when selecting previously inserted text.
const VK_RIGHT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x27);

/// Virtual key code for the Shift key.
///
/// Used as a selection modifier.
const VK_SHIFT_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0x10);

/// Presses Ctrl, taps the provided virtual key, then releases Ctrl.
///
/// Returns `true` if all input events were successfully sent.
pub fn send_ctrl_combo(vk: VIRTUAL_KEY) -> bool {
    let mut seq = KeySequence::new();
    seq.down(VK_CONTROL) && KeySequence::tap(vk)
}

/// A small RAII helper that tracks pressed keys and releases them on drop.
///
/// Intended for modifier keys (Ctrl, Shift, Alt). If `down` succeeds, the key is
/// recorded and will be released in reverse order when the sequence is dropped.
pub struct KeySequence {
    pressed: Vec<VIRTUAL_KEY>,
}

impl KeySequence {
    /// Creates an empty key sequence.
    pub fn new() -> Self {
        Self {
            pressed: Vec::new(),
        }
    }

    /// Sends a key down event and records the key for automatic release.
    ///
    /// Returns `true` if the event was successfully sent.
    pub fn down(&mut self, vk: VIRTUAL_KEY) -> bool {
        send_key(vk, false).then(|| self.pressed.push(vk)).is_some()
    }

    /// Taps a key (down then up).
    ///
    /// Returns `true` if both events were successfully sent.
    pub fn tap(vk: VIRTUAL_KEY) -> bool {
        send_key(vk, false) && send_key(vk, true)
    }
}

impl Default for KeySequence {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KeySequence {
    fn drop(&mut self) {
        for vk in self.pressed.drain(..).rev() {
            let _ = send_key(vk, true);
        }
    }
}

/// Sends a Unicode string using `KEYEVENTF_UNICODE` keyboard input.
///
/// Notes:
/// - The Windows API consumes UTF-16 code units. Supplementary characters are sent
///   as surrogate pairs (two units), which is expected for `KEYEVENTF_UNICODE`.
/// - Returns `true` if all input events were successfully sent.
pub fn send_text_unicode(text: &str) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_UNICODE;

    fn unicode_input(unit: u16, key_up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: if key_up {
                        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                    } else {
                        KEYEVENTF_UNICODE
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    let units: Vec<u16> = text.encode_utf16().collect();
    if units.is_empty() {
        return true;
    }

    let inputs: Vec<INPUT> = units
        .into_iter()
        .flat_map(|u| [unicode_input(u, false), unicode_input(u, true)])
        .collect();

    let Some(input_size) = input_struct_size_i32() else {
        return false;
    };

    let sent = unsafe { SendInput(&inputs, input_size) } as usize;
    sent == inputs.len()
}

/// Reselects the last inserted text by moving the caret left and selecting right.
///
/// `units` is the number of UTF-16 code units to reselect. This matches the unit
/// count produced by `str::encode_utf16()`.
///
/// Behavior:
/// - Move caret left `units` times
/// - Hold Shift
/// - Move caret right `units` times (selecting the range)
///
/// The selection length is capped to avoid excessive key events.
pub fn reselect_last_inserted_text_utf16_units(units: usize) -> bool {
    const MAX_UNITS: usize = 4096;

    if units == 0 {
        return true;
    }

    let units = units.min(MAX_UNITS);
    let mut seq = KeySequence::new();

    (0..units).all(|_| KeySequence::tap(VK_LEFT_KEY))
        && seq.down(VK_SHIFT_KEY)
        && (0..units).all(|_| KeySequence::tap(VK_RIGHT_KEY))
}

fn input_struct_size_i32() -> Option<i32> {
    i32::try_from(std::mem::size_of::<INPUT>()).ok()
}

/// Sends a single virtual key event via `SendInput`.
///
/// `key_up = false` sends key down, `key_up = true` sends key up.
///
/// Returns `true` if `SendInput` reports that at least one event was inserted.
fn send_key(vk: VIRTUAL_KEY, key_up: bool) -> bool {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let Some(input_size) = input_struct_size_i32() else {
        return false;
    };

    let sent = unsafe { SendInput(&[input], input_size) };
    usize::try_from(sent).is_ok_and(|n| n != 0)
}
