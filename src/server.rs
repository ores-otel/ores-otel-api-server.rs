#![forbid(unsafe_code)]

use crate::config::ApiConfig;
use crate::routes;

pub fn run(config: &ApiConfig) {
    println!("api bind {}", config.bind);
    match serde_json::to_string(&routes::rest::health::body()) {
        Ok(body) => println!("{body}"),
        Err(error) => eprintln!("health response serialization failed: {error}"),
    }
}
