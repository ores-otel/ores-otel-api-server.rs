#![forbid(unsafe_code)]

/// Canonical standalone-server WebSocket upgrade ingress.
///
/// WebSocket transport reuses already-admitted semantic operations and does
/// not create a fourth Lambda leaf family or WebSocket-specific callable IDs.
pub const PATH: &str = "/ws";
pub const INGRESS_KIND: &str = "ws";
