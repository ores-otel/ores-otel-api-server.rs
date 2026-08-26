#![forbid(unsafe_code)]

use serde::Serialize;

#[derive(Serialize)]
pub struct HealthBody {
    pub ok: bool,
    pub service: &'static str,
}

pub fn body() -> HealthBody {
    HealthBody { ok: true, service: "ores-otel-api-server" }
}

