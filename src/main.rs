#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Split the application into a set of focused modules.  Each module
// encapsulates a distinct aspect of the program: application state
// (`app`), small helper routines (`helpers`), user interface layout
// (`ui`), visual styling (`visuals`) and the Windows message loop
// (`win`).  The `mod` declarations below make the modules available
// throughout the crate.
mod app;
mod helpers;
mod ui;
mod visuals;
mod win;

/// Entry point for the program.  Ensures only a single instance is
/// running and then delegates window creation and the message loop to
/// the `win` module.  Any error returned from the window code is
/// propagated to the OS so that a meaningful HRESULT is reported.
fn main() -> windows::core::Result<()> {
    // Acquire a guard which prevents multiple instances of the
    // application from running concurrently.  If another instance is
    // already active the process will exit immediately.
    let _guard = helpers::single_instance_guard()?;

    // Launch the UI and enter the message loop.  All further
    // initialization is handled in `win::run`.
    win::run()
}