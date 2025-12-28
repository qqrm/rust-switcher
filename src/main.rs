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
        return Ok(());
    };

    platform::win::run()
}

#[cfg(test)]
mod tests;
