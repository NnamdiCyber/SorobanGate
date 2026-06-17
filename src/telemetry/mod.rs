/// Optional OpenTelemetry tracing setup (OTLP export).
/// Currently a no-op stub; wire up the `opentelemetry` + `opentelemetry-otlp` crates
/// here when v1.1 tracing is enabled.
pub fn init_tracing(_otlp_endpoint: Option<&str>) {
    // No-op: tracing_enabled = false in default config.
    // When enabled, initialise an OTLP exporter here and set it as the global tracer.
}

pub fn shutdown_tracing() {
    // Flush and shut down any global tracer provider here.
}
