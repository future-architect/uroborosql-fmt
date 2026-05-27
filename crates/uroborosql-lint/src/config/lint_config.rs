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
    },
    File {
        path: String,
    },
}
