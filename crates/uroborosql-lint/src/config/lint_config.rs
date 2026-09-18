use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LintConfigObject {
    #[serde(default)]
    pub db: Option<DbConfig>,
    #[serde(default)]
    pub rules: HashMap<String, Value>,
    #[serde(default)]
    pub overrides: Vec<LintOverride>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LintOverride {
    pub files: Vec<String>,
    #[serde(default)]
    pub rules: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "schemaProvider", rename_all = "lowercase")]
pub enum DbConfig {
    Server {
        host: String,
        #[serde(default)]
        port: Option<u16>,
        user: String,
        #[serde(default)]
        password: Option<String>,
        #[serde(rename = "dbname")]
        dbname: String,
        #[serde(default, rename = "tlsMode")]
        tls_mode: ConfigTlsMode,
        #[serde(default)]
        timeouts: ConfigTimeouts,
    },
    File {
        path: String,
    },
}

/// Configuration remains independent of optional driver features.
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigTlsMode {
    #[default]
    VerifyFull,
    VerifyCa,
    Require,
    Disable,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigTimeouts {
    pub connect_ms: Option<u64>,
    pub query_ms: Option<u64>,
    pub acquisition_ms: Option<u64>,
}
