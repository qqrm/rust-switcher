#[cfg(feature = "debug-tracing")]
use tracing_subscriber::EnvFilter;

#[cfg(feature = "debug-tracing")]
pub fn init_tracing() {
    if !cfg!(debug_assertions) {
        return;
    }

    let default_filter = "trace";
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_level(true)
        .with_target(true);

    if subscriber.try_init().is_ok() {
        tracing::info!("tracing initialized");
    }
}

#[cfg(not(feature = "debug-tracing"))]
pub fn init_tracing() {}
