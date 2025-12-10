use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Default, Deserialize, Clone, JsonSchema)]
#[serde(default)]
pub struct Configuration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db: Option<DbConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<HashMap<String, RuleLevel>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Vec<OverrideConfig>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[serde(tag = "schemaProvider")]
pub enum DbConfig {
    #[serde(rename = "server")]
    Server(ServerConfig),
    #[serde(rename = "file")]
    File(FileConfig),
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub struct ServerConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub struct FileConfig {
    pub path: String,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub struct OverrideConfig {
    pub files: Vec<String>,
    pub rules: HashMap<String, RuleLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleLevel {
    Error,
    Warn,
    Off,
}
