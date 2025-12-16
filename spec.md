# Rust Switcher – Specification

## Purpose

**Rust Switcher** is a small background utility for **Windows 11** that performs an explicit, user-triggered keyboard layout conversion.

The application **does not automatically fix input**.
It executes a user command (**DoubleShift**) to:

* convert already typed text
* switch the system keyboard layout
* continue typing in the new layout

---

## Target Platform and Scope

* OS: **Windows 11**
* Works in regular windowed applications:

  * browsers
  * IDEs
  * messengers
* Not required to work in:

  * fullscreen applications
  * games
  * borderless fullscreen windows

---

## Single Hotkey

### DoubleShift

Definition:

* Two consecutive Shift presses (keydown + keyup)
* **No other keys** may be pressed between them
* If any of the following are involved, it **does not trigger**:

  * Ctrl
  * Alt
  * Win
  * any other key
* `Shift + Alt + Shift` is **not** DoubleShift

Timing:

* Fixed default timeout
* Optionally configurable via GUI (slider)

---

## DoubleShift Behavior

### Case 1: Text is selected

1. Send `Ctrl+C`
2. Read text from the **system clipboard**
3. Switch system keyboard layout to the **next layout**
4. Convert text according to the new layout
5. Send `Ctrl+V`
6. Restore original clipboard contents

---

### Case 2: No selection

1. Select the last word to the left of the cursor

   * stop at whitespace or start of line
2. Apply the same flow as in **Case 1**

Notes:

* Uses **system clipboard**
* Clipboard is always backed up and restored
* No per-character backspace logic

---

## Text Conversion

* Simple character mapping based on keyboard layout
* No language detection
* No heuristics
* No “correct” layout guessing

Logic:

* take existing text
* switch system layout
* transform characters
* insert result

Supports:

* any number of layouts
* layout cycling via system round-robin

---

## System Keyboard Layout

* Layout **is always switched** on each DoubleShift
* Switches to the **next system layout**
* The new layout remains active for further typing

The application:

* does **not** override Alt+Shift or Win+Space
* relies on standard Windows layout switching

---

## Tray and GUI

### Tray Icon

* Always present while the application is running

Right click:

* Pause / Resume
* Exit

Left click:

* Show / hide GUI

---

### GUI (Minimal)

* DoubleShift timeout (slider)
* Autostart (checkbox)
* Pause / Resume toggle

No additional settings.

---

## Autostart

* Portable application model
* When enabled:

  * executable is copied to `%APPDATA%\RustSwitcher\`
  * `config.json` is stored there
  * autostart points to that executable

---

## Configuration

* `%APPDATA%\RustSwitcher\config.json`
* JSON format
* Atomic writes

---

## Logging and Networking

* Release builds:

  * no logs
  * no networking
  * no telemetry
* Debug builds:

  * optional debug logging

---

## Stability and Safety

* Single instance only
* Self-generated input events are ignored
* If any modifier is pressed, DoubleShift does not trigger

---

## Out of Scope

* AI features
* API-based translation
* smart language detection
* multiple hotkeys
* macOS / Linux (for now)

---

## Definition of Done

* DoubleShift reliably works in browsers and IDEs
* Selection and last-word cases both work
* System layout actually switches
* Tray and autostart work
* No unnecessary behavior
