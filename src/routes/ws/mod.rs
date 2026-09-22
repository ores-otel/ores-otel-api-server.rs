#![forbid(unsafe_code)]

pub mod v1;

// WebSocket is standalone ingress only; it never owns semantic Lambda leaves.
