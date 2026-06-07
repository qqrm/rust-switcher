#[cfg(debug_assertions)]
pub const APP_DIR: &str = "RustSwitcherDebug";
#[cfg(not(debug_assertions))]
pub const APP_DIR: &str = "RustSwitcher";

#[cfg(debug_assertions)]
pub const APP_USER_MODEL_ID: &str = "RustSwitcher.Debug";
#[cfg(not(debug_assertions))]
pub const APP_USER_MODEL_ID: &str = "RustSwitcher";

#[cfg(debug_assertions)]
pub const MAIN_WINDOW_CLASS: &str = "RustSwitcherDebugMainWindow";
#[cfg(not(debug_assertions))]
pub const MAIN_WINDOW_CLASS: &str = "RustSwitcherMainWindow";

#[cfg(debug_assertions)]
pub const SHOW_MAIN_WINDOW_MESSAGE: &str = "RustSwitcher.Debug.ShowMainWindow";
#[cfg(not(debug_assertions))]
pub const SHOW_MAIN_WINDOW_MESSAGE: &str = "RustSwitcher.ShowMainWindow";

#[cfg(debug_assertions)]
pub const SINGLE_INSTANCE_MUTEX: &str = "Global\\RustSwitcherDebug_SingleInstance";
#[cfg(not(debug_assertions))]
pub const SINGLE_INSTANCE_MUTEX: &str = "Global\\RustSwitcher_SingleInstance";
