use std::sync::LazyLock;

use figment::providers::Format;
use figment::{Figment, providers::Toml};

fn load_properties() -> Properties {
    let config_path = std::env::var("TS3STATUS_CONFIG_PATH")
        .expect("env var TS3STATUS_CONFIG_PATH must be set to the path of a TOML config file");

    // file_exact, not file: a path that doesn't exist must be an error rather
    // than an empty config that later fails with a confusing "missing field".
    Figment::new()
        .merge(Toml::file_exact(&config_path))
        .extract::<Properties>()
        .unwrap_or_else(|e| panic!("Failed to load configuration from {config_path}: {e}"))
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct Properties {
    pub database_path: String,
    pub ts3_host: String,
    pub ts3_user: String,
    pub ts3_pass: String,
    pub ts3_nick: String,
}

pub static PROPERTIES: LazyLock<Properties> = LazyLock::new(load_properties);
