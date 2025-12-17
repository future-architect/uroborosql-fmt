use serde::Deserialize;
use std::collections::HashMap;

use crate::LintError;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ConfigurationObject {
    pub db: Option<DbConfig>,

    pub rules: Option<HashMap<String, RuleLevel>>,

    pub overrides: Option<Vec<OverrideConfig>>,

    pub ignore: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum DbConfig {
    #[serde(rename = "server")]
    Server(ServerConfig),
    #[serde(rename = "file")]
    File(FileConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileConfig {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OverrideConfig {
    pub files: Vec<String>,
    pub rules: HashMap<String, RuleLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleLevel {
    Error,
    Warn,
    Off,
}

pub fn deserialize_config(config_json: &str) -> Result<ConfigurationObject, LintError> {
    serde_json::from_str(config_json)
        .map_err(|e| LintError::ConfigurationError(e.to_string()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_config() {
        let config_json = r#"
            {
                "db": {
                    "server": {
                        "host": "localhost",
                        "port": 5432,
                        "database": "my_database",
                        "user": "my_user",
                        "password": "my_password"
                    }
                },
                "rules": {
                    "rule-id": "warn"
                },
                "overrides": [
                    {
                        "files": ["src/*.sql"],
                        "rules": {
                            "global-rule": "error"
                        }
                    }
                ],
                "ignore": ["src/ignored.sql"]
            }
        "#;

        let config: ConfigurationObject = serde_json::from_str(config_json).unwrap();
        assert_eq!(
            config.db,
            Some(DbConfig::Server(ServerConfig {
                host: "localhost".to_string(),
                port: 5432,
                database: "my_database".to_string(),
                user: "my_user".to_string(),
                password: "my_password".to_string(),
            }))
        );
        assert_eq!(
            config.rules,
            Some(HashMap::from([("rule-id".to_string(), RuleLevel::Warn)]))
        );
        assert_eq!(
            config.overrides,
            Some(vec![OverrideConfig {
                files: vec!["src/*.sql".to_string()],
                rules: HashMap::from([("global-rule".to_string(), RuleLevel::Error)]),
            }])
        );
        assert_eq!(config.ignore, Some(vec!["src/ignored.sql".to_string()]));
    }
}
