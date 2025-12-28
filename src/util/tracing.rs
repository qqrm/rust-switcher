use std::sync::Mutex;

use tracing_appender::non_blocking::WorkerGuard;

static TRACING_GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

pub fn init_tracing() {
    let file_appender = tracing_appender::rolling::hourly("./logs", "output.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_level(true)
        .with_target(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    store_tracing_guard(_guard);
}

fn store_tracing_guard(_guard: WorkerGuard) {
    *TRACING_GUARD.lock().unwrap() = Some(_guard);
}
