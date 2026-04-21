use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fs::File;
use std::io::BufReader;
use std::sync::RwLock;

use crate::error::UroboroSQLFmtError;

/// 設定を保持するグローバル変数
pub(crate) static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| RwLock::new(Config::default()));

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Case {
    Upper,
    Lower,
    Preserve,
}

impl Default for Case {
    /// Caseのデフォルト値(lower)
    fn default() -> Self {
        Case::Lower
    }
}

impl Case {
    pub(crate) fn format(&self, key: &str) -> String {
        match self {
            Case::Upper => key.to_uppercase(),
            Case::Lower => key.to_lowercase(),
            Case::Preserve => key.to_string(),
        }
    }
}

/// ユーザーが明示的に指定したオプションのみを保持する構造体。
///
/// - 設定ファイル (`.uroborosqlfmtrc.json`) とオーバーライド JSON を型付きでマージする際の中間表現
/// - `ClientConfig` (LSP) からも flatten して再利用することで、フィールド列挙を 1 箇所に集約
/// - snake_case (設定ファイル) と camelCase (VSCode 等の LSP クライアント) の両方で deserialize 可能
///   にするため、複数単語フィールドには `#[serde(alias)]` で camelCase の別名を付与している
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PartialConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "tabSize")]
    pub tab_size: Option<usize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "complementAlias"
    )]
    pub complement_alias: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "trimBindParam"
    )]
    pub trim_bind_param: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "keywordCase"
    )]
    pub keyword_case: Option<Case>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "identifierCase"
    )]
    pub identifier_case: Option<Case>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "maxCharPerLine"
    )]
    pub max_char_per_line: Option<isize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "complementOuterKeyword"
    )]
    pub complement_outer_keyword: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "complementColumnAsKeyword"
    )]
    pub complement_column_as_keyword: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "removeTableAsKeyword"
    )]
    pub remove_table_as_keyword: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "removeRedundantNest"
    )]
    pub remove_redundant_nest: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "complementSqlId"
    )]
    pub complement_sql_id: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "convertDoubleColonCast"
    )]
    pub convert_double_colon_cast: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "unifyNotEqual"
    )]
    pub unify_not_equal: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "indentTab")]
    pub indent_tab: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "useParserErrorRecovery"
    )]
    pub use_parser_error_recovery: Option<bool>,
}

impl PartialConfig {
    /// `other` の値で `self` を上書きする (other 優先)。
    pub fn merge(self, other: PartialConfig) -> PartialConfig {
        PartialConfig {
            debug: other.debug.or(self.debug),
            tab_size: other.tab_size.or(self.tab_size),
            complement_alias: other.complement_alias.or(self.complement_alias),
            trim_bind_param: other.trim_bind_param.or(self.trim_bind_param),
            keyword_case: other.keyword_case.or(self.keyword_case),
            identifier_case: other.identifier_case.or(self.identifier_case),
            max_char_per_line: other.max_char_per_line.or(self.max_char_per_line),
            complement_outer_keyword: other
                .complement_outer_keyword
                .or(self.complement_outer_keyword),
            complement_column_as_keyword: other
                .complement_column_as_keyword
                .or(self.complement_column_as_keyword),
            remove_table_as_keyword: other
                .remove_table_as_keyword
                .or(self.remove_table_as_keyword),
            remove_redundant_nest: other.remove_redundant_nest.or(self.remove_redundant_nest),
            complement_sql_id: other.complement_sql_id.or(self.complement_sql_id),
            convert_double_colon_cast: other
                .convert_double_colon_cast
                .or(self.convert_double_colon_cast),
            unify_not_equal: other.unify_not_equal.or(self.unify_not_equal),
            indent_tab: other.indent_tab.or(self.indent_tab),
            use_parser_error_recovery: other
                .use_parser_error_recovery
                .or(self.use_parser_error_recovery),
        }
    }

    /// `None` のフィールドをそれぞれの既定値で埋めて、確定値 `Config` を生成する。
    pub fn resolve(self) -> Config {
        Config {
            debug: self.debug.unwrap_or(false),
            tab_size: self.tab_size.unwrap_or(4),
            complement_alias: self.complement_alias.unwrap_or(true),
            trim_bind_param: self.trim_bind_param.unwrap_or(false),
            keyword_case: self.keyword_case.unwrap_or_default(),
            identifier_case: self.identifier_case.unwrap_or_default(),
            max_char_per_line: self.max_char_per_line.unwrap_or(50),
            complement_outer_keyword: self.complement_outer_keyword.unwrap_or(true),
            complement_column_as_keyword: self.complement_column_as_keyword.unwrap_or(true),
            remove_table_as_keyword: self.remove_table_as_keyword.unwrap_or(true),
            remove_redundant_nest: self.remove_redundant_nest.unwrap_or(true),
            complement_sql_id: self.complement_sql_id.unwrap_or(false),
            convert_double_colon_cast: self.convert_double_colon_cast.unwrap_or(true),
            unify_not_equal: self.unify_not_equal.unwrap_or(true),
            indent_tab: self.indent_tab.unwrap_or(true),
            use_parser_error_recovery: self.use_parser_error_recovery.unwrap_or(true),
        }
    }
}

/// 確定値の設定構造体 (formatter 内部用)。
///
/// `PartialConfig::resolve()` で生成される。外部から直接 deserialize する用途はないため、
/// serde trait は付けていない。
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) debug: bool,
    pub(crate) tab_size: usize,
    pub(crate) complement_alias: bool,
    pub(crate) trim_bind_param: bool,
    pub(crate) keyword_case: Case,
    pub(crate) identifier_case: Case,
    pub(crate) max_char_per_line: isize,
    pub(crate) complement_outer_keyword: bool,
    pub(crate) complement_column_as_keyword: bool,
    pub(crate) remove_table_as_keyword: bool,
    pub(crate) remove_redundant_nest: bool,
    pub(crate) complement_sql_id: bool,
    pub(crate) convert_double_colon_cast: bool,
    pub(crate) unify_not_equal: bool,
    pub(crate) indent_tab: bool,
    pub(crate) use_parser_error_recovery: bool,
}

impl Config {
    /// 設定ファイルより優先度の高い設定を記述した JSON 文字列と、設定ファイルのパスから Config 構造体を生成する。
    ///
    /// Returns `Config` that is created from the json string which describes higher priority options
    /// than the configuration file and the configuration file path.
    pub fn new(
        settings_json: Option<&str>,
        config_path: Option<&str>,
    ) -> Result<Config, UroboroSQLFmtError> {
        let base = match config_path {
            Some(path) => {
                let file = File::open(path).map_err(|_| {
                    UroboroSQLFmtError::FileNotFound("Setting file not found".to_string())
                })?;
                let reader = BufReader::new(file);
                serde_json::from_reader::<_, PartialConfig>(reader)
                    .map_err(|e| UroboroSQLFmtError::IllegalSettingFile(e.to_string()))?
            }
            None => PartialConfig::default(),
        };

        let override_ = match settings_json {
            Some(json) => serde_json::from_str::<PartialConfig>(json).map_err(|e| {
                UroboroSQLFmtError::Runtime(format!("Setting json is invalid. {e}"))
            })?,
            None => PartialConfig::default(),
        };

        Ok(base.merge(override_).resolve())
    }
}

impl Default for Config {
    fn default() -> Self {
        PartialConfig::default().resolve()
    }
}

/// 引数に与えた Config 構造体をグローバル変数 CONFIG に読み込む
pub(crate) fn load_settings(config: Config) {
    *CONFIG.write().unwrap() = config;
}

/// 補完・削除を行わない設定をロードする
pub(crate) fn load_never_complement_settings() {
    let config = PartialConfig {
        complement_alias: Some(false),
        trim_bind_param: Some(false),
        complement_sql_id: Some(false),
        complement_outer_keyword: Some(false),
        complement_column_as_keyword: Some(false),
        remove_table_as_keyword: Some(false),
        remove_redundant_nest: Some(false),
        convert_double_colon_cast: Some(false),
        unify_not_equal: Some(false),
        ..PartialConfig::default()
    }
    .resolve();

    *CONFIG.write().unwrap() = config;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_camel_case_deserialize() {
        let json = r#"{"tabSize":2,"keywordCase":"upper"}"#;
        let partial: PartialConfig = serde_json::from_str(json).unwrap();
        assert_eq!(partial.tab_size, Some(2));
        assert_eq!(partial.keyword_case, Some(Case::Upper));
        assert_eq!(partial.identifier_case, None);
    }

    #[test]
    fn partial_config_snake_case_deserialize() {
        let json = r#"{"tab_size":2,"keyword_case":"upper"}"#;
        let partial: PartialConfig = serde_json::from_str(json).unwrap();
        assert_eq!(partial.tab_size, Some(2));
        assert_eq!(partial.keyword_case, Some(Case::Upper));
    }

    #[test]
    fn partial_config_merge_other_wins() {
        let base = PartialConfig {
            tab_size: Some(4),
            keyword_case: Some(Case::Lower),
            ..PartialConfig::default()
        };
        let override_ = PartialConfig {
            keyword_case: Some(Case::Upper),
            ..PartialConfig::default()
        };
        let merged = base.merge(override_);
        assert_eq!(merged.tab_size, Some(4));
        assert_eq!(merged.keyword_case, Some(Case::Upper));
    }

    #[test]
    fn partial_config_resolve_uses_defaults() {
        let config = PartialConfig::default().resolve();
        assert!(!config.debug);
        assert_eq!(config.tab_size, 4);
        assert!(config.complement_alias);
        assert!(!config.trim_bind_param);
        assert_eq!(config.max_char_per_line, 50);
    }
}
