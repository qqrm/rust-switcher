use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;

static TRACING_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

pub fn init_tracing() {
    let file_appender = tracing_appender::rolling::hourly("./logs", "output.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_level(true)
        .with_target(true)
        .finish();

    // If a global subscriber is already set (e.g. init called twice), do nothing.
    if tracing::subscriber::set_global_default(subscriber).is_ok() {
        let _ = TRACING_GUARD.set(guard);
    }
}
