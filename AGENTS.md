# AGENTS.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Overview

Rust Switcher is a Windows 11 utility for converting text between Russian (ЙЦУКЕН) and English (QWERTY) keyboard layouts. It's a native Win32 GUI application using the `windows` crate directly (no framework).

## Build Commands

This project requires **Rust nightly** (specified in `rust-toolchain.toml`) and **MSVC toolchain**.

```powershell
# Development build with debug tracing
cargo +nightly build --features debug-tracing

# Release build
cargo +nightly build --release --locked

# Run tests
cargo +nightly test --locked

# Format check
cargo +nightly fmt --check

# Clippy (mirrors CI)
cargo +nightly clippy --all-targets --all-features -- -D warnings
```

### Bacon (recommended for development)

The project uses `bacon` for a fast development loop. Run `bacon` and use keybindings:
- `d` — dev-long: fmt check, clippy, debug build, run app
- `r` — release-long: fmt check, clippy, release build, run app
- `t` — test-long: fmt check, clippy, run tests
- `p` — dushnota: maximum strictness clippy (includes `clippy::pedantic`)

### Enable logging

```powershell
$env:RUST_LOG="trace"
cargo +nightly run -F debug-tracing
```

## Code Style Requirements

### Strict Clippy Lints

The codebase enforces strict linting. These are **denied** (will fail CI):
- `clippy::unwrap_used` — use `ok()`, `map()`, `?` operator, or explicit error handling
- `clippy::expect_used` — same as above
- `clippy::todo`, `clippy::unimplemented` — no TODO/unimplemented macros in committed code
- `clippy::dbg_macro` — no debug macros

Exception: Tests (in `src/tests/`) allow `unwrap_used` and `expect_used`.

### Formatting

Configured in `rustfmt.toml`:
- `imports_granularity = "Crate"` — group imports by crate
- `group_imports = "StdExternalCrate"` — std first, then external, then local

## Architecture

### Module Structure

```
src/
├── main.rs           # Entry point, single-instance guard
├── app.rs            # AppState struct (all UI state, control handles)
├── config/           # Config load/save, validation, hotkey definitions
├── domain/text/      # Core conversion logic
│   ├── mapping.rs    # RU↔EN character mapping (convert_ru_en_bidirectional)
│   ├── convert.rs    # Selection conversion, layout switching
│   └── last_word.rs  # Last-word conversion logic
├── conversion/       # High-level conversion API
│   ├── clipboard.rs  # Clipboard operations
│   └── input.rs      # Input simulation
├── input/            # Input handling
│   ├── hotkeys.rs    # Win32 hotkey registration (RegisterHotKey)
│   └── ring_buffer.rs# Keystroke buffer for last-word tracking
├── platform/
│   ├── win/          # Windows implementation
│   │   ├── window.rs # Window creation, message loop
│   │   ├── keyboard/ # Keyboard hooks (WH_KEYBOARD_LL)
│   │   ├── tray.rs   # System tray icon
│   │   └── ...
│   └── ui/           # UI components
│       ├── themes.rs # Light/dark theme support
│       └── ...
└── utils/            # Helpers, tracing setup
```

### Key Patterns

**AppState**: Central state struct stored in window user data (`GWLP_USERDATA`). Access via `with_state_mut(hwnd, |state| ...)`.

**Error handling**: Uses `ui_call!` and `ui_try!` macros to push errors to a notification queue rather than panicking. Errors are shown to users via balloon notifications.

**Hotkey system**: Two modes:
1. Traditional hotkeys via `RegisterHotKey` (single chord)
2. Chord sequences (e.g., double-tap Shift) tracked via `HotkeySequence` and runtime state

**Config**: JSON at `%APPDATA%\RustSwitcher\config.json`. Loaded via `confy` crate. Always validate with `cfg.validate_hotkey_sequences()` before saving.

### Win32 Specifics

- Window class: `RustSwitcherMainWindow`
- Uses `WH_KEYBOARD_LL` hook for keystroke capture (in `keyboard/capture.rs`)
- Uses `WH_MOUSE_LL` hook for mouse events
- Tray icon via `Shell_NotifyIconW`
- Dark mode via `DwmSetWindowAttribute` and custom `WM_CTLCOLOR*` handling

## Testing

Tests are in `src/tests/`. Run a single test:

```powershell
cargo +nightly test test_name -- --nocapture
```

Test modules cover: config I/O, config validation, hotkey formatting, keyboard sequences, character mapping invariants, ring buffer.

## Agent Workflow Requirements

When making changes in this repository, always:

- Run formatting, linting, build, and tests before reporting results:
  - `cargo +nightly fmt --check`
  - `cargo +nightly clippy --all-targets --all-features -- -D warnings`
  - `cargo +nightly build --features debug-tracing`
  - `cargo +nightly test --locked`
- Address and fix any findings from these checks before finalizing work.
