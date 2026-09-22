#![forbid(unsafe_code)]

use axum::Router;

use crate::AppState;

pub mod graphql;
pub mod rest;
pub mod rpc;
pub mod ws;

/// Compose the currently implemented HTTP route projections.
///
/// The RPC, GraphQL, and WebSocket trees remain ingress-only authorities until
/// their runtime adapters are wired; semantic leaves stay under their separate
/// governed roots rather than being mirrored into `src/routes/**`.
pub fn router() -> Router<AppState> {
    Router::new().merge(rest::router())
}
