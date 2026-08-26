#![forbid(unsafe_code)]

use ores_otel_api_server::{config::ApiConfig, server};

fn main() {
    let cfg = ApiConfig::from_env();
    server::run(&cfg);
}

