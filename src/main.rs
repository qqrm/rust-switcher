#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![feature(stmt_expr_attributes)]

mod app;
mod config;
mod conversion;
mod domain;
mod helpers;
mod input;
mod input_journal;
mod platform;
mod utils;

fn main() -> windows::core::Result<()> {
    utils::tracing::init_tracing();
    utils::helpers::init_app_user_model_id()?;

    let Some(_guard) = utils::helpers::single_instance_guard()? else {
        let _ = platform::win::activate_running_instance();
        return Ok(());
    };

    let autostart_hidden = std::env::args().any(|arg| arg == platform::win::AUTOSTART_ARG);

    let cfg_hidden = config::load()
        .ok()
        .map(|c| c.start_minimized)
        .unwrap_or(false);

    let start_hidden = autostart_hidden || cfg_hidden;

    platform::win::run(start_hidden)
}

#[cfg(test)]
mod tests;
