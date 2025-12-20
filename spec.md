# Rust Switcher - Specification

## Purpose

Rust Switcher is a small background utility for Windows 11 that performs explicit, user triggered keyboard layout conversion.

The application does not try to automatically fix input. All actions are executed only by user commands.

Supported actions:
- Convert last word: convert the last word to the left of the caret
- Convert selection: convert the currently selected text
- Switch keyboard layout: switch the system layout only, without converting text
- Pause: temporarily disable all hotkey handling
- Toggle conversion scope: toggle Convert last word scope between Last word and Last phrase

---

## Target Platform and Scope

- OS: Windows 11
- Works in regular windowed applications:
  - browsers
  - IDEs
  - messengers
- Not required to work in:
  - fullscreen applications
  - games
  - borderless fullscreen windows

---

## Hotkeys

The application uses a set of hotkeys configurable via the GUI.

### Actions

1) Convert last word
- If there is a selection, behaves like Convert selection
- If there is no selection, selects the last word to the left of the caret and converts it

2) Convert selection
- Requires a non empty selection, otherwise does nothing

3) Switch keyboard layout
- Always switches the system keyboard layout to the next layout
- Does not modify any text

4) Pause
- Toggles pause state on or off
- When paused, other hotkeys do nothing

5) Toggle conversion scope
- Toggles Convert last word scope between Last word and Last phrase
- Trigger: pressing Left Shift and Right Shift together
- Must not interfere with normal typing
- Debounced: triggers once per press pair until at least one Shift is released

### Default Hotkeys

Defaults match the reference UI:
- Convert last word: Pause
- Convert selection: Ctrl + Break (sometimes shown as Control + Cancel)
- Switch keyboard layout: None
- Pause: None

Notes:
- On assignment, if the hotkey is unavailable or conflicts, the action must not become active and the previous value remains.
- The application does not override Alt + Shift or Win + Space and relies on standard Windows layout switching.

---

## Execution Delay

Setting: Delay before paste (ms)
- A delay before inserting the converted text
- Needed for stability in apps with slow selection or clipboard updates
- Configured as an integer in milliseconds
- Default value: 100 ms

---

## Text Conversion

Base:
- Character mapping based on keyboard layouts (RU <-> EN for the active pair)
- Conversion is deterministic for a given mapping table
- No translation
- No API calls
- No background auto-fix

Two scopes for Convert last word:
- Last word: convert only the last word to the left of the caret
- Last phrase: greedily extend the range by words to the left while conversion quality improves, then convert that range

Last phrase heuristic:
- Uses lingua-rs limited to languages: Russian and English
- The app compares how plausible the text looks before and after conversion
- The app extends selection left by one word at a time and stops when:
  - conversion no longer improves the target language confidence by a threshold, or
  - confidence drops below a minimal threshold, or
  - max_words limit is reached
- The heuristic runs only on explicit user command, never automatically

Logic for conversion:
- take the existing text (selection)
- convert characters according to the mapping table (no system layout switching is required for conversion itself)
- insert the result

Layout switching:
- Switching the system layout is a separate step and can be performed after conversion so the user continues typing in the expected layout


---

## Action Flows

### Convert selection

1) Send Ctrl + C
2) Read text from the system clipboard
3) Wait Delay before paste
4) Convert the text according to the mapping table
5) Send Ctrl + V
6) Restore original clipboard contents

Requirements:
- After paste, switch system keyboard layout to the next layout so the user continues typing in the expected layout
- Clipboard is always backed up and restored
- No per character backspace logic

### Convert last word

If there is a selection:
- behaves like Convert selection

If there is no selection:
- depends on Conversion scope

Scope: Last word
1) Select the last word to the left of the caret
   - stop at whitespace or start of line
2) Then run Convert selection flow

Scope: Last phrase
1) Select the last word to the left of the caret
2) Repeat:
   - evaluate original vs converted text with lingua-rs (RU, EN only)
   - if converted is better for the same target language, extend selection by one word to the left
   - otherwise stop and revert to the last good selection
3) Then run Convert selection flow

Limits:
- max_words: N (configurable, default 8)
- minimal confidence and minimal delta thresholds are configurable constants

---

## Tray and GUI

### Tray Icon

Setting: Show tray icon
- If enabled, the tray icon is always present while the application is running
- If disabled, there is no tray icon, and the GUI can only be opened from the window

Tray context menu:
- Pause or Resume
- Exit

Tray click:
- Show or hide GUI

### GUI

The settings window is minimal and matches the reference layout.

Left side:
- Checkbox: Start on windows startup
- Checkbox: Show tray icon
- Input: Delay before switching (ms)
- Combo: Conversion scope
  - Last word
  - Last phrase
- Button: Report an issue
- Button: Exit program

Right side, Hotkeys group:
- Read only display fields showing current hotkeys:
  - Convert last word
  - Pause
  - Convert selection
  - Switch keyboard layout
- Must support the value None

Bottom buttons:
- Apply
  - atomically writes the config and applies settings
- Cancel
  - discards UI changes and restores values from the current config
  

### Report an issue

- Opens the project issues page using the system shell
- The application itself does not send any data and has no telemetry

---

## Autostart

Portable application model.

When Start on windows startup is enabled:
- the executable is copied to `%APPDATA%\RustSwitcher\`
- `config.json` is stored there
- autostart points to that copied executable

When disabled:
- the autostart entry is removed
- files under `%APPDATA%\RustSwitcher\` are not removed automatically

---

## Configuration

- Path: `%APPDATA%\RustSwitcher\config.json`
- Format: JSON
- Atomic writes (write to temp then rename)
- Settings:
  - start_on_startup: bool
  - show_tray_icon: bool
  - delay_ms: u32
  - hotkey_convert_last_word: Hotkey or None
  - hotkey_convert_selection: Hotkey or None
  - hotkey_switch_layout: Hotkey or None
  - hotkey_pause: Hotkey or None
  - paused: bool
  - conversion_scope: "last_word" | "last_phrase"
  - last_phrase_max_words: u32

---

## Logging and Networking

- Release builds:
  - no logs
  - no networking
  - no telemetry
- Debug builds:
  - optional debug logging

---

## Stability and Safety

- Single instance only
- Self generated input events are ignored
- When paused, no hotkeys except Pause are executed
- Clipboard is always restored even on errors (best effort)

---

## Out of Scope

- AI features
- API based translation
- smart language detection
- macOS or Linux (for now)

---

## Definition of Done

- Convert last word works reliably in browsers and IDEs
- Convert selection works reliably in browsers and IDEs
- Switch keyboard layout actually switches the system layout
- Tray, pause or resume, and autostart work
- GUI matches the spec, Apply or Cancel works correctly
- No unnecessary behavior
