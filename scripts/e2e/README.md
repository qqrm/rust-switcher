# Interactive E2E

These checks exercise the real Windows keyboard hook, foreground focus, and
`SendInput` behavior in an isolated Windows Sandbox desktop.

They are intentionally separate from normal unit tests because they require an
interactive Windows session. A regular container or non-interactive service
session cannot reliably validate `WH_KEYBOARD_LL`, foreground-window focus, or
global input injection.

## Run

From the repository root:

```powershell
.\scripts\e2e\run-sandbox.ps1
```

The host script:

1. Builds `rust-switcher.exe` with `debug-tracing`.
2. Creates a temporary bundle under `%TEMP%`.
3. Launches Windows Sandbox with that bundle mapped to the sandbox desktop.
4. Runs `sandbox-runner.ps1` inside the sandbox.
5. Waits for the sandbox to shut down and prints `result.json`.

## Scenario Covered

The current scenario validates the cursor-regression path:

1. Start Rust Switcher with an isolated `%APPDATA%` and deterministic
   `Shift+F12` convert hotkey.
2. Open an isolated WinForms multiline `TextBox`.
3. Type `ghbdtn world`.
4. Press `Left` five times, putting the caret before `world`.
5. Press `Shift+F12`.
6. Assert the text becomes `привет world`.

## Requirements

- Windows 11 host.
- Windows Sandbox feature enabled.
- Rust nightly/MSVC available on the host for the build step.

The sandbox is shut down automatically after the run. Use `-KeepOpen` while
debugging the sandbox desktop.
