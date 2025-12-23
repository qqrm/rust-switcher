#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod config_validator;
mod conversion;
mod helpers;
mod hotkeys;
mod input_journal;
mod tray;
mod ui;
mod visuals;
mod win;
mod constants;

pub fn init_tracing() {
    use std::sync::Once;

    use tracing_subscriber::{EnvFilter, fmt, fmt::format::FmtSpan, prelude::*};

    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        let file_appender = tracing_appender::rolling::never("logs", "rust-switcher.log");
        let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

        // Важно: _guard должен жить до конца процесса.
        // Если у тебя нет глобального места, положи guard в static Mutex<Option<Guard>>.
        store_tracing_guard(_guard);

        let fmt_console = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT);

        let fmt_file = fmt::layer()
            .with_writer(file_writer)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_console)
            .with(fmt_file)
            .init();

        tracing::info!("tracing initialized");
    });
}

// Пример хранения guard. Реализуй в своем модуле.
fn store_tracing_guard(guard: tracing_appender::non_blocking::WorkerGuard) {
    use std::sync::Mutex;
    static GUARD: Mutex<Option<tracing_appender::non_blocking::WorkerGuard>> = Mutex::new(None);
    *GUARD.lock().unwrap() = Some(guard);
}

fn main() -> windows::core::Result<()> {
    init_tracing();
    helpers::init_app_user_model_id()?;

    let Some(_guard) = helpers::single_instance_guard()? else {
        return Ok(());
    };

    win::run()
}
