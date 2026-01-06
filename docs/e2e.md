# E2E (MVP) scenarios

This document captures the minimal end-to-end checks we want to keep stable.

## MVP scenarios

1. **Launch app**
   - Build the executable (`cargo +nightly build` or `cargo build`).
   - Start the app and ensure the process stays alive.
2. **Main window appears**
   - Find the window by `Name = RustSwitcher`.
3. **Key UI state change**
   - Update a primary UI state (MVP: **"Delay before switching"** field).
   - Verify the value changes.
4. **Status/controls visible**
   - Verify core controls are visible (e.g. "Apply" button and hotkey labels like
     "Autoconvert pause:").

## WinAppDriver setup

Install WinAppDriver on the Windows runner or dev box.

### Command to start WinAppDriver

```powershell
# Default port 4723
& "C:\Program Files (x86)\Windows Application Driver\WinAppDriver.exe" 4723/wd/hub
```

### Desired capabilities

```json
{
  "platformName": "Windows",
  "deviceName": "WindowsPC",
  "app": "C:\\path\\to\\target\\x86_64-pc-windows-msvc\\debug\\rust-switcher.exe"
}
```

## Running the E2E test locally

```powershell
pip install -r tests/e2e/requirements.txt
$env:RUST_SWITCHER_EXE = "C:\\path\\to\\target\\x86_64-pc-windows-msvc\\debug\\rust-switcher.exe"
$env:WINAPPDRIVER_URL = "http://127.0.0.1:4723/wd/hub"
pytest tests/e2e
```

## CI notes

E2E requires an interactive Windows desktop session so that UI Automation can
attach to the window. The CI workflow starts WinAppDriver, builds the app, and
executes the tests using the same capabilities above.
