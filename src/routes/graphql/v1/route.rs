#![forbid(unsafe_code)]

/// Canonical HTTP ingress for GraphQL execution planning.
///
/// Semantic GraphQL leaves remain under `src/graphql/**/resolver.rs`; this
/// route namespace never mirrors resolver hierarchy or owns callable identity.
pub const PATH: &str = "/v1/graphql";
pub const INGRESS_KIND: &str = "graphql";
