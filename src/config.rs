#![forbid(unsafe_code)]

use crate::env_map::{value, EnvMap};

#[derive(Clone, Debug)]
pub struct ApiConfig {
    pub bind: String,
    pub tcp_bind: Option<String>,
    pub nats_url: Option<String>,
}

impl ApiConfig {
    pub fn from_env_map(env: &EnvMap) -> Self {
        Self {
            bind: value(env, crate::env::BIND)
                .unwrap_or("127.0.0.1:8080")
                .to_owned(),
            tcp_bind: value(env, crate::env::TCP_BIND).map(str::to_owned),
            nats_url: value(env, crate::env::NATS_URL).map(str::to_owned),
        }
    }

    pub fn from_env() -> Self {
        let env = crate::env::load().unwrap_or_else(|err| panic!("{err}"));
        Self::from_env_map(&env)
    }
}
