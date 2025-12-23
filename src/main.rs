#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod conversion;
mod helpers;
mod hotkeys;
mod input_journal;
mod tray;
mod ui;
mod visuals;
mod win;
mod config_validator;

fn init_tracing() {
    #[cfg(feature = "debug-tracing")]
    {
        use tracing_subscriber::{EnvFilter, fmt};

        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        fmt().with_env_filter(filter).compact().init();
    }
}

fn main() -> windows::core::Result<()> {
    init_tracing();
    helpers::init_app_user_model_id()?;

    let Some(_guard) = helpers::single_instance_guard()? else {
        return Ok(());
    };

    win::run()
}
