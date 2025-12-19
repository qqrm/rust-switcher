#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod conversion;
mod helpers;
mod hotkeys;
mod tray;
mod ui;
mod visuals;
mod win;

fn main() -> windows::core::Result<()> {
    helpers::init_app_user_model_id()?;

    let _guard = helpers::single_instance_guard()?;

    win::run()
}
