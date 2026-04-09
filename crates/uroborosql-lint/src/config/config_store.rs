use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::Value;
use thiserror::Error;

use crate::{
    diagnostic::Severity,
    rules::{all_rules, default_rules, RuleEnum},
};

use super::{
    lint_config::{DbConfig, LintConfigObject, LintOverride},
    overrides::ResolvedOverride,
    RuleLevel, RuleSetting, DEFAULT_CONFIG_FILENAME,
};

#[derive(Debug, Clone)]
pub enum ResolvedDbConfig {
    Server {
        host: String,
        port: Option<u16>,
        user: String,
        password: Option<String>,
        dbname: String,
    },
    File {
        path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct ResolvedLintConfig {
    pub rules: Vec<(RuleEnum, Severity)>,
    pub db: Option<ResolvedDbConfig>,
}

impl Default for ResolvedLintConfig {
    fn default() -> Self {
        Self {
            rules: default_rules(),
            db: None,
        }
    }
}

#[derive(Debug, Clone)]
struct LintConfig {
    rules: HashMap<String, RuleSetting>,
    overrides: Vec<ResolvedOverride>,
    ignore: GlobSet,
    db: Option<ResolvedDbConfig>,
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    root_dir: PathBuf,
    unresolved_config: LintConfig,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read {0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("failed to parse {0}: {1}")]
    Json(PathBuf, #[source] serde_json::Error),
    #[error("invalid glob pattern {0}: {1}")]
    Glob(String, #[source] globset::Error),
    #[error("invalid rule setting for {rule}: {value}")]
    InvalidRuleSetting { rule: String, value: Value },
}

impl ConfigStore {
    pub fn new(
        root_dir: impl Into<PathBuf>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        let root_dir = root_dir.into();
        let (lint_config_object, origin) = load_config(&root_dir, config_path)?;
        let unresolved_config =
            LintConfig::from_lint_config_object(lint_config_object, &root_dir, origin.clone())?;
        Ok(Self {
            root_dir,
            unresolved_config,
        })
    }

    pub fn resolve(&self, file: &Path) -> ResolvedLintConfig {
        let rel_path = file.strip_prefix(&self.root_dir).unwrap_or(file);
        let mut rules = self.unresolved_config.rules.clone();

        // override を順番に適用（後勝ち）
        for override_config in &self.unresolved_config.overrides {
            if override_config.files.is_match(rel_path) {
                for (name, setting) in &override_config.rules {
                    rules.insert(name.clone(), setting.clone());
                }
            }
        }

        // rule と severity のペアを作成
        let mut resolved_rules = Vec::new();
        for rule in all_rules() {
            let name = rule.name();

            let severity = if let Some(setting) = rules.get(name) {
                match setting.level {
                    RuleLevel::Off => continue,
                    RuleLevel::Warn => Severity::Warning,
                    RuleLevel::Error => Severity::Error,
                }
            } else {
                rule.default_severity()
            };

            resolved_rules.push((rule, severity));
        }

        ResolvedLintConfig {
            rules: resolved_rules,
            db: self.unresolved_config.db.clone(),
        }
    }

    pub fn is_ignored(&self, file: &Path) -> bool {
        let rel_path = file.strip_prefix(&self.root_dir).unwrap_or(file);
        self.unresolved_config.ignore.is_match(rel_path)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }
}

impl LintConfig {
    fn from_lint_config_object(
        lint_config_object: LintConfigObject,
        root_dir: &Path,
        origin: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        let rules = parse_rules_map(lint_config_object.rules)?;
        let overrides = lint_config_object
            .overrides
            .into_iter()
            .map(resolve_override)
            .collect::<Result<Vec<_>, _>>()?;
        let ignore = build_globset(&lint_config_object.ignore)?;
        let db = resolve_db_config(lint_config_object.db, root_dir, origin.as_deref());

        Ok(Self {
            rules,
            overrides,
            ignore,
            db,
        })
    }
}

fn load_config(
    root_dir: &Path,
    config_path: Option<PathBuf>,
) -> Result<(LintConfigObject, Option<PathBuf>), ConfigError> {
    if let Some(path) = config_path {
        let config = read_config_file(&path)?;
        return Ok((config, Some(path)));
    }

    let path = root_dir.join(DEFAULT_CONFIG_FILENAME);
    if path.exists() {
        let config = read_config_file(&path)?;
        return Ok((config, Some(path)));
    }

    Ok((LintConfigObject::default(), None))
}

fn read_config_file(path: &Path) -> Result<LintConfigObject, ConfigError> {
    let content =
        fs::read_to_string(path).map_err(|err| ConfigError::Io(path.to_path_buf(), err))?;
    let config =
        serde_json::from_str(&content).map_err(|err| ConfigError::Json(path.to_path_buf(), err))?;
    Ok(config)
}

fn resolve_override(ov: LintOverride) -> Result<ResolvedOverride, ConfigError> {
    let files = build_globset(&ov.files)?;
    let rules = parse_rules_map(ov.rules)?;
    Ok(ResolvedOverride { files, rules })
}

fn resolve_db_config(
    config: Option<DbConfig>,
    root_dir: &Path,
    origin: Option<&Path>,
) -> Option<ResolvedDbConfig> {
    let base_dir = origin.and_then(|path| path.parent()).unwrap_or(root_dir);
    match config? {
        DbConfig::Server {
            host,
            port,
            user,
            password,
            dbname,
        } => Some(ResolvedDbConfig::Server {
            host,
            port,
            user,
            password,
            dbname,
        }),
        DbConfig::File { path } => Some(ResolvedDbConfig::File {
            path: base_dir.join(path),
        }),
    }
}

fn build_globset(patterns: &[String]) -> Result<GlobSet, ConfigError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|err| ConfigError::Glob(pattern.clone(), err))?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|err| ConfigError::Glob("failed to build globset".to_string(), err))
}

fn parse_rules_map(
    rules: HashMap<String, Value>,
) -> Result<HashMap<String, RuleSetting>, ConfigError> {
    let mut parsed = HashMap::new();
    for (name, value) in rules {
        let setting = parse_rule_setting(&name, &value)?;
        parsed.insert(name, setting);
    }
    Ok(parsed)
}

fn parse_rule_setting(name: &str, value: &Value) -> Result<RuleSetting, ConfigError> {
    let level = parse_rule_level(value).ok_or_else(|| ConfigError::InvalidRuleSetting {
        rule: name.to_string(),
        value: value.clone(),
    })?;
    Ok(RuleSetting { level })
}

fn parse_rule_level(value: &Value) -> Option<RuleLevel> {
    match value {
        Value::String(text) => match text.as_str() {
            "off" => Some(RuleLevel::Off),
            "warn" | "warning" => Some(RuleLevel::Warn),
            "error" => Some(RuleLevel::Error),
            "0" => Some(RuleLevel::Off),
            "1" => Some(RuleLevel::Warn),
            "2" => Some(RuleLevel::Error),
            _ => None,
        },
        Value::Number(num) => match num.as_i64()? {
            0 => Some(RuleLevel::Off),
            1 => Some(RuleLevel::Warn),
            2 => Some(RuleLevel::Error),
            _ => None,
        },
        _ => None,
    }
}
