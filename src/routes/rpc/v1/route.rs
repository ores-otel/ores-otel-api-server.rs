#![forbid(unsafe_code)]

/// Canonical HTTP ingress for the aggregate RPC registry.
///
/// Semantic RPC leaves remain under `src/rpc/**/funcs.rs`; REST-derived RPC
/// projections retain ownership in their `src/routes/rest/**` leaf.
pub const PATH: &str = "/v1/rpc";
pub const INGRESS_KIND: &str = "rpc";
