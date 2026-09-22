#![forbid(unsafe_code)]

use axum::Router;

use crate::AppState;

pub mod health {
    pub mod handlers;
    pub mod route;
}

pub mod v1 {
    pub mod handlers;
    pub mod route;
}

/// Compose the authored REST projections admitted under `src/routes/rest/**`.
///
/// RPC, GraphQL, and WebSocket namespaces are separate ingress authorities and
/// are intentionally not discovered recursively through this REST router.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::route::router())
        .merge(v1::route::router())
}
