//! An app's logs and traces: stderr always, and an OTLP collector when one
//! is named (`OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, or
//! `OTEL_EXPORTER_OTLP_ENDPOINT`). Nothing leaves the machine otherwise.
//!
//! ```ignore
//! fn main() {
//!     let _telemetry = app_shell_native::telemetry::init("drive-app", "warn,drive=info");
//!     app_shell_native::launch(App, popout, config);
//! }
//! ```
//!
//! `RUST_LOG` overrides the default filter. The guard flushes the
//! exporter when it drops (at the end of `main`).

use opentelemetry::trace::TracerProvider as _;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// Keeps the exporter flushing until `main` ends.
pub struct Guard(Option<opentelemetry_sdk::trace::SdkTracerProvider>);

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let _ = p.shutdown();
        }
    }
}

fn named(var: &str) -> bool {
    std::env::var(var).is_ok_and(|v| !v.trim().is_empty())
}

/// Whether spans go to a collector.
#[must_use]
pub fn exporting() -> bool {
    named("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT") || named("OTEL_EXPORTER_OTLP_ENDPOINT")
}

/// Install the app's subscriber, as `service` (once, first thing in `main`).
#[must_use]
pub fn init(service: &str, default_filter: &str) -> Guard {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| default_filter.into());
    let provider = exporting()
        .then(|| {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .build()
                .ok()?;
            Some(
                opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_batch_exporter(exporter)
                    .with_resource(
                        opentelemetry_sdk::Resource::builder()
                            .with_service_name(service.to_owned())
                            .build(),
                    )
                    .build(),
            )
        })
        .flatten();
    let otel = provider
        .as_ref()
        .map(|p| tracing_opentelemetry::layer().with_tracer(p.tracer("app-shell")));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(otel)
        .try_init();
    Guard(provider)
}
