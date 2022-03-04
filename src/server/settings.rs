use config::{Config, ConfigError, File};
use serde::Deserialize;

use std::fmt;

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

const CONFIG_FILE_PATH: &str = "src/server/settings/Default.json";
const CONFIG_FILE_PREFIX: &str = "src/server/settings/";

#[derive(Clone, Debug, Deserialize)]
pub enum ENV {
    Testing,
    Development,
    Production,
}

impl fmt::Display for ENV {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ENV::Testing => write!(f, "Testing"),
            ENV::Production => write!(f, "Production"),
            ENV::Development => write!(f, "Development"),
        }
    }
}

impl From<&str> for ENV {
    fn from(env: &str) -> Self {
        match env {
            "Testing" => ENV::Testing,
            "Production" => ENV::Production,
            _ => ENV::Development,
        }
    }
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let env = std::env::var("RUN_ENV").unwrap_or_else(|_| "Development".into());
        let settings = Config::builder()
            .set_default("env", env.clone())
            .unwrap()
            .add_source(File::with_name(CONFIG_FILE_PATH))
            .add_source(File::with_name(&format!("{}{}", CONFIG_FILE_PREFIX, env)))
            .build()
            .unwrap()
            .try_deserialize();

        settings
    }
}
