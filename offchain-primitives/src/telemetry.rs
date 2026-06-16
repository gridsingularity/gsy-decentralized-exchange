use tracing::subscriber::set_global_default;
use tracing::Subscriber;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

pub fn build_telemetry<Sink>(
    name: String,
    env_filter: String,
    sink: Sink,
) -> impl Subscriber + Send + Sync
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name, sink);
    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

pub fn init_telemetry(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}

/// Builds and installs a stdout telemetry subscriber in one step.
///
/// Convenience wrapper that pairs [`build_telemetry`] with [`init_telemetry`],
/// writing to `std::io::stdout`.
///
/// * `name` - Label attached to every log record, typically the service name.
/// * `log_level` - Fallback level filter (e.g. `"info"`) used when `RUST_LOG`
///   is not set.
pub fn setup_telemetry(name: impl Into<String>, log_level: impl Into<String>) {
    let telemetry = build_telemetry(name.into(), log_level.into(), std::io::stdout);
    init_telemetry(telemetry);
}