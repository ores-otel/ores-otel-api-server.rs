#![forbid(unsafe_code)]
//! Shared `ores-transport` adapter for the API server's network avenues.
//!
//! Direct read-only database access is owned by the web server. The three
//! avenues that reach this process—HTTP envelopes, stateful TCP, and
//! JetStream—must all dispatch through the same [`ores_transport::OperationHandler`]
//! so operation semantics cannot drift by wire protocol.

use ores_transport::{
    Envelope, NatsSubjects, OperationHandler, Reply, ServeError, serve_envelope,
};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;

/// Service slug used to derive subjects and stream names.
pub const SERVICE_SLUG: &str = "ores-otel";
/// Environment prefix for transport configuration.
pub const ENV_PREFIX: &str = "ORES_OTEL";
/// HTTP envelope route owned by the API transport plane.
pub const ENVELOPE_PATH: &str = "/v1/operations";

/// Startup/serving failures at the shared transport boundary.
///
/// Keep the public adapter typed even where `ores-transport` currently exposes
/// an erased JetStream setup error internally. That prevents this service from
/// leaking a catch-all error contract into its own call sites.
#[derive(Debug, thiserror::Error)]
pub enum SharedTransportError {
    #[error("transport configuration is invalid: {0}")]
    Config(#[from] ores_transport::ConfigError),
    #[error("transport connection failed: {0}")]
    Connect(#[from] ores_transport::TransportError),
    #[error("jetstream serving failed: {0}")]
    JetStream(String),
}

#[must_use]
pub fn subjects() -> NatsSubjects {
    NatsSubjects::for_service(SERVICE_SLUG)
}

/// Dispatch an HTTP envelope through the same domain handler used by TCP and
/// JetStream.
pub async fn serve_http_envelope<O, T>(
    handler: &dyn OperationHandler<O, T>,
    envelope: &Envelope<O>,
) -> (Reply<T>, Result<(), ServeError>)
where
    O: Send + Sync,
    T: Send,
{
    serve_envelope(handler, envelope).await
}

/// Serve stateful internal TCP connections with the shared operation handler.
pub async fn serve_stateful<O, T>(
    listener: tokio::net::TcpListener,
    handler: Arc<dyn OperationHandler<O, T>>,
) where
    O: DeserializeOwned + Send + Sync + 'static,
    T: Serialize + Send + 'static,
{
    ores_transport::serve_tcp(listener, handler).await;
}

/// Serve the asynchronous JetStream avenue with the shared operation handler.
pub async fn serve_asynchronous<O, T>(
    context: ores_transport::async_nats::jetstream::Context,
    handler: Arc<dyn OperationHandler<O, T>>,
) -> Result<(), SharedTransportError>
where
    O: DeserializeOwned + Send + Sync,
    T: Serialize + Send,
{
    ores_transport::serve_jetstream(context, subjects(), handler)
        .await
        .map_err(|error| SharedTransportError::JetStream(error.to_string()))
}

/// Build a JetStream context from the canonical `ORES_OTEL_*` transport
/// configuration. Missing NATS configuration disables this avenue rather than
/// inventing a fallback endpoint.
pub async fn jetstream_from_env() -> Result<
    Option<ores_transport::async_nats::jetstream::Context>,
    SharedTransportError,
> {
    let config = ores_transport::TransportConfig::from_env(ENV_PREFIX)?;
    match config.nats_url.as_deref() {
        None => Ok(None),
        Some(url) => Ok(Some(ores_transport::connect_nats(url).await?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_slug_matches_environment_prefix() {
        assert_eq!(SERVICE_SLUG.replace('-', "_").to_uppercase(), ENV_PREFIX);
    }

    #[test]
    fn subjects_are_namespaced_and_stream_safe() {
        let subjects = subjects();
        assert!(subjects.request_subject.starts_with(SERVICE_SLUG));
        assert!(subjects.result_subject.starts_with(SERVICE_SLUG));
        assert!(!subjects.request_stream.contains(['.', '-']));
        assert!(!subjects.result_stream.contains(['.', '-']));
    }
}
