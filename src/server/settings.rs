use config::{Config, ConfigError, File};
use serde::Deserialize;

use std::path::Path;

use crate::zfx_id::Id;

// For explanation, see issue: https://github.com/serde-rs/serde/issues/368
fn default_true() -> bool {
    true
}
fn default_cert() -> Option<String> {
    Some("node.crt".to_string())
}
fn default_key() -> Option<String> {
    Some("node.key".to_string())
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Settings {
    pub id: Option<Id>,
    pub listener_ip: String,
    pub bootstrap_peers: Vec<String>,
    pub keypair: String,
    #[serde(default = "default_true")]
    pub use_tls: bool,
    #[serde(default = "default_cert")]
    pub certificate_file: Option<String>,
    #[serde(default = "default_key")]
    pub private_key_file: Option<String>,
}

const CONFIG_NAME: &str = "config.json";
const CONFIG_PREFIX: &str = "src/server/settings/";

impl Settings {
    pub fn new(home: Option<&str>) -> Result<Self, ConfigError> {
        let home_dir = if let Some(hd) = home { hd } else { CONFIG_PREFIX };
        let config_path = Path::new(&home_dir).join(CONFIG_NAME);

        let settings = Config::builder()
            .add_source(File::with_name(config_path.to_str().unwrap()))
            .build()
            .unwrap()
            .try_deserialize();

        settings
    }
}
