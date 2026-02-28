# Rust Switcher - Specification (implementation aligned)

This document describes the current behavior and architecture of the repository implementation.
It is intended as onboarding documentation and as a reference for expected runtime behavior.

## Scope

- Supported OS: Windows (tested on Windows 11)
- Primary UI: native Win32 window + tray icon
- Linux: out of scope for runtime support (some parts may compile, but the app is not a supported Linux product)

## Core user goals

- Convert text typed in the wrong keyboard layout (RU <-> EN) quickly and reliably.
- Work without relying on clipboard paste for the main path.
- Provide global hotkeys and a lightweight UI for configuration.

## Terminology

- Convert: map characters between keyboard layouts (RU <-> EN) using the built in mapping table.
- Selection: currently selected text in the active application.
- Last word: the last token captured by the keyboard hook input journal.
- Autoconvert: automatic conversion triggered by typed delimiter characters (for example Space).
- Autoconvert enabled: runtime flag controlling whether Autoconvert is active.
  - Important: this flag is NOT persisted in config.
  - Default on app start: disabled.

## High level architecture

- UI boundary (Win32):
  - Window procedure, message loop, controls
  - Apply and Cancel logic
  - Tray icon integration and tray menu actions
- Input boundary:
  - Low level keyboard hook (WH_KEYBOARD_LL)
  - Input journal and ring buffer for tokenization and last word extraction
  - Hotkey sequences matching
- Domain logic:
  - Text conversion, replacement of selection, insertion via SendInput
- Platform integration:
  - Global hotkeys are implemented by a hybrid model:
    - RegisterHotKey for legacy single-chord bindings
    - WH_KEYBOARD_LL + sequence matcher for hotkey sequences
  - Keyboard layout switching
  - Autostart shortcut in Startup folder
  - Notifications (tray balloon, MessageBox fallback)
- Persistence:
  - Config stored under %APPDATA%\RustSwitcher\config.json via confy

## Configuration

Config fields (see src/config.rs):

- delay_ms: u32
- start_minimized: bool
- theme_dark: bool

Hotkeys (legacy single chord, optional):
- hotkey_convert_last_word
- hotkey_convert_selection
- hotkey_switch_layout
- hotkey_pause

Hotkey sequences (preferred, optional):
- hotkey_convert_last_word_sequence
- hotkey_pause_sequence
- hotkey_convert_selection_sequence
- hotkey_switch_layout_sequence

Notes:
- Autoconvert enabled is runtime only and is not stored in config.
- Hotkey fields are shown in read-only edits, but user interaction updates pending sequence values that are applied on Apply.
- Hotkey sequences are validated on save.

Default bindings (current defaults in code):
- Convert smart: double tap Left Shift within 1000 ms
- Autoconvert toggle: press Left Shift + Right Shift together
- Switch keyboard layout: CapsLock
- Convert selection: configured but by default it is the same double tap Left Shift.
  Since sequence matching checks Convert smart earlier, Convert selection is effectively shadowed unless rebound to a different sequence.

## Actions and behavior

### Convert smart

This is the primary conversion action.

Behavior:
- If there is a non empty selection, it converts the selection.
- Otherwise it converts last sequence using the input journal.

### Convert selection

Algorithm (src/domain/text/convert.rs and clipboard helper module):
- Copy selection text while restoring clipboard afterwards (best effort, clipboard snapshot restore).
- Sleep for autoconvert_delay_ms before conversion and replacement.
- Convert the copied text via mapping.
- Replace selection by:
  - Send Delete to remove the selection
  - Inject Unicode text via SendInput
  - Attempt to reselect the inserted text within a retry budget

This intentionally avoids paste via Ctrl+V to reduce interference with application specific paste behavior.

### Convert last sequence

Algorithm (src/domain/text/last_word.rs):
- Uses the input journal tokenization to determine the last sequence.
- Sleep for autoconvert_delay_ms before conversion and replacement.
- Applies an input based replacement strategy (backspace and Unicode injection via SendInput).
- Clipboard is not used as the primary mechanism.

### Switch keyboard layout

Switches keyboard layout (Windows) for the current thread using the platform API.

### Autoconvert

- The low level keyboard hook maintains a ring buffer of recent tokens.
- When a trigger delimiter is typed, the hook posts a window message (WM_APP_AUTOCONVERT).
- The UI thread handles WM_APP_AUTOCONVERT and calls autoconvert_last_word only when Autoconvert enabled is true.
- A guard prevents double conversion of the same token.

### Autoconvert toggle

- The toggle hotkey flips runtime Autoconvert enabled.
- Autoconvert enabled default is disabled on app start.
- Toggling shows an informational tray balloon.
- Tray double click triggers the same toggle.

## UI

Native Win32 UI with a single window and two group sections (custom painted frames):

### Settings group
- Autostart (checkbox)
- Delay ms (edit box)
- Theme dark (checkbox)

### Hotkeys group
- Read only displays for:
  - Convert smart (sequence)
  - Convert selection (sequence)
  - Autoconvert toggle (sequence)
  - Switch layout (sequence)

Buttons:
- Apply: persists config and applies theme changes immediately
- Cancel: reloads config from disk and applies it to UI and runtime (including theme)
- Exit: closes the application

Note: there is currently no dedicated GitHub/"Report issue" button in the shipped UI.

Theme behavior:
- Theme can be changed from UI checkbox + Apply.
- Theme can also be changed from tray menu.
- Tray theme switch persists theme_dark into config immediately (without overwriting other pending UI edits).

## Tray icon

- A tray icon is always added via Shell_NotifyIconW.
- Right click shows a context menu:
  - Toggle autoconvert
  - Show or Hide (toggles window visibility)
  - Change theme
  - Exit
- Left click is implemented:
  - Single click toggles window visibility (debounced with a timer to distinguish it from double click).
- Double click is implemented:
  - Toggles autoconvert.

## Autostart

- Implemented by creating a shortcut RustSwitcher.lnk in the user Startup folder.
- The shortcut points to the current executable path.
- Moving or deleting the executable breaks autostart.

Persistence note:
- Autostart is NOT stored in config.json.
- The UI checkbox reflects the current system autostart shortcut presence.
- Toggling the checkbox creates or deletes the shortcut immediately.

## Notifications and errors

- Info notifications use tray balloon where possible.
- Error notifications are queued and drained on the UI thread via a single entry point.
- Fallback for tray failure is MessageBoxW.
- Notifications must not block hotkey critical paths.

## Logging

Current behavior:
- Tracing initialization exists for development only.
- When enabled, logs go to stderr.
- By default, release builds do not install any tracing subscriber.

How it is gated:
- Tracing initialization is guarded by debug assertions.
- The intended way to enable logs during development is to build and run with feature debug-tracing and use RUST_LOG.

## Known issues (current behavior)

- Convert selection default sequence duplicates Convert smart and is shadowed unless user rebinds it.
- Autostart depends on a shortcut pointing to the current exe path, so relocating the exe breaks autostart.
