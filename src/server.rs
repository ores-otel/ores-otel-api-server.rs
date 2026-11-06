#![forbid(unsafe_code)]
use std::future::Future;
use crate::{config::ApiConfig, routes, AppState};

/// Use one listener for every authored route projection and let the caller own shutdown.
/// The listener is injected by E2E tests to avoid a port-race and assert real TCP behavior.
pub async fn serve_with_listener(
    listener: tokio::net::TcpListener,
    config: ApiConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let app = routes::router().with_state(AppState { config });
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
}

/// Fail closed on bind/runtime errors instead of merely printing a health sample.
pub fn run(config: &ApiConfig) -> std::io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(config.bind.as_str()).await?;
        serve_with_listener(listener, config.clone(), std::future::pending()).await
    })
}
