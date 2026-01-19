use std::{collections::HashMap, fs::File, sync::LazyLock};

fn load_properties() -> HashMap<String, String> {
    // TODO: pass this in an argument or env var or something
    // also TODO: use a different config format
    let f = File::open("/run/agenix/ts3status.properties").unwrap();
    java_properties::read(f).unwrap()
}

pub static PROPERTIES: LazyLock<HashMap<String, String>> = LazyLock::new(|| load_properties());
