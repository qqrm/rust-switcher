#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![feature(stmt_expr_attributes)]

use crate::util::tracing::init_tracing;

mod app;
mod config;
mod config_validator;
mod constants;
mod conversion;
mod helpers;
mod hotkeys;
mod input_journal;
mod tray;
mod ui;
mod util;
mod visuals;
mod win;

fn main() -> windows::core::Result<()> {
    init_tracing();
    helpers::init_app_user_model_id()?;

    let Some(_guard) = helpers::single_instance_guard()? else {
        return Ok(());
    };

    win::run()
}

#[cfg(test)]
mod tests;
