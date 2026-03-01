# AGENTS.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Overview

Rust Switcher is a Windows 11 utility for converting text between Russian (ЙЦУКЕН) and English (QWERTY) keyboard layouts. It's a native Win32 GUI application using the `windows` crate directly (no framework).

## Toolchain

- Pinned toolchain: Rust 1.93.1 (see `rust-toolchain.toml`)
- MSRV policy: Rust 1.93 (see `rust-version` in `Cargo.toml`)
- Requires MSVC toolchain (Visual Studio 2022 Build Tools)

```powershell
# Development build with debug tracing
cargo build --features debug-tracing

# Release build
cargo build --release --locked

# Run tests
cargo test --workspace --all-features --all-targets --locked

# Format check
cargo fmt --all -- --check

# Clippy (strict; mirrors CI)
cargo clippy --workspace --all-targets --all-features --   -D warnings   -W clippy::all   -W clippy::pedantic   -W clippy::nursery   -W clippy::cargo   -W clippy::perf   -A clippy::multiple_crate_versions
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
cargo run -F debug-tracing
```

## Code Style Requirements

### Strict Clippy

CI runs strict clippy including `clippy::pedantic`, `clippy::nursery`, `clippy::cargo`, and `clippy::perf`.

Exception: `clippy::multiple_crate_versions` is allowed because it is usually Cargo.lock noise and often not actionable.

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
│   ├── mapping.rs    # RU↔EN character mapping (re-exported from rust-switcher-core)
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

## Testing

Tests are in `src/tests/`. Run a single test:

```powershell
cargo test test_name -- --nocapture
```

## Agent Workflow Requirements

When making changes in this repository, always:

- Run formatting, linting, build, and tests before reporting results:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --all-features -- ...`
  - `cargo build --features debug-tracing`
  - `cargo test --workspace --all-features --all-targets --locked`

- Address and fix any findings from these checks before finalizing work.
