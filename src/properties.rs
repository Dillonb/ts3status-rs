use std::sync::LazyLock;

use figment::{Figment, providers::Toml};
use figment::providers::Format;

fn load_properties() -> Properties {
    let config_location =
        std::env::var("TS3STATUS_CONFIG_PATH").expect("env var TS3STATUS_CONFIG_PATH must be set");
    let figment = Figment::new().merge(Toml::file(config_location));

    figment
        .extract::<Properties>()
        .expect("Failed to load configuration")
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

pub static PROPERTIES: LazyLock<Properties> = LazyLock::new(|| load_properties());
