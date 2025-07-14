use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fs::File;
use std::io::BufReader;
use std::sync::RwLock;

use crate::error::UroboroSQLFmtError;

/// 設定を保持するグローバル変数
pub(crate) static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| RwLock::new(Config::default()));

/// debugのデフォルト値(false)
fn default_debug() -> bool {
    false
}

/// tab_sizeのデフォルト値(4)
fn default_tab_size() -> usize {
    4
}

/// complement_aliasのデフォルト値(true)
fn default_complement_alias() -> bool {
    true
}

/// trim_bind_paramのデフォルト値(false)
fn default_trim_bind_param() -> bool {
    false
}

/// max_char_per_lineのデフォルト値(50)
fn default_max_char_per_line() -> isize {
    50
}

/// complement_outer_keywordのデフォルト値(true)
fn default_complement_outer_keyword() -> bool {
    true
}

/// complement_as_keywordのデフォルト値(true)
fn default_complement_column_as_keyword() -> bool {
    true
}

/// remove_table_as_keywordのデフォルト値(true)
fn default_remove_table_as_keyword() -> bool {
    true
}

/// remove_redundant_nestのデフォルト値(true)
fn default_remove_redundant_nest() -> bool {
    true
}

/// complement_sql_idのデフォルト値(false)
fn default_complement_sql_id() -> bool {
    false
}

/// convert_double_colon_castのデフォルト値(true)
fn default_convert_double_colon_cast() -> bool {
    true
}

/// unify_not_equalのデフォルト値(true)
fn default_unify_not_equal() -> bool {
    true
}

/// indent_tabのデフォルト値(true)
fn default_indent_tab() -> bool {
    true
}

fn default_use_parser_error_recovery() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Case {
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

/// 設定を保持する構造体
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    /// デバッグモード
    #[serde(default = "default_debug")]
    pub(crate) debug: bool,
    /// タブ幅
    #[serde(default = "default_tab_size")]
    pub(crate) tab_size: usize,
    /// カラムエイリアスがない場合にエイリアス名を自動的に補完する
    #[serde(default = "default_complement_alias")]
    pub(crate) complement_alias: bool,
    /// バインド変数の中身をトリムする
    #[serde(default = "default_trim_bind_param")]
    pub(crate) trim_bind_param: bool,
    /// キーワードを大文字・小文字にする
    #[serde(default = "Case::default")]
    pub(crate) keyword_case: Case,
    /// 識別子を大文字・小文字にする
    #[serde(default = "Case::default")]
    pub(crate) identifier_case: Case,
    /// 1行当たりの文字数上限 (タブを含まない)
    #[serde(default = "default_max_char_per_line")]
    pub(crate) max_char_per_line: isize,
    /// OUTER キーワードの自動補完を有効にする
    ///
    /// このオプションで補完されるキーワードは、keyword_case = "preserve"のとき、
    /// コーディング規約に合わせて大文字とする。
    /// 将来的には、keyword_case オプションで補完するキーワードのcaseを、
    /// preserve_complement_upper (補完は大文字)、preserve_complement_lower (補完は小文字)、...
    /// のように設定できるようにしてもよい。
    #[serde(default = "default_complement_outer_keyword")]
    pub(crate) complement_outer_keyword: bool,
    /// カラムエイリアスにおける AS キーワードの自動補完を有効にする
    #[serde(default = "default_complement_column_as_keyword")]
    pub(crate) complement_column_as_keyword: bool,
    /// テーブルエイリアスにおける AS キーワードの自動除去を有効にする
    #[serde(default = "default_remove_table_as_keyword")]
    pub(crate) remove_table_as_keyword: bool,
    /// 余分な括弧を自動で除去する
    #[serde(default = "default_remove_redundant_nest")]
    pub(crate) remove_redundant_nest: bool,
    /// /* _SQL_ID_ */がない場合に自動で補完する
    #[serde(default = "default_complement_sql_id")]
    pub(crate) complement_sql_id: bool,
    /// `X::type`のキャストを`CAST(X AS type)`に変換する
    #[serde(default = "default_convert_double_colon_cast")]
    pub(crate) convert_double_colon_cast: bool,
    /// not_equalを!=に統一する
    #[serde(default = "default_unify_not_equal")]
    pub(crate) unify_not_equal: bool,
    /// 空白文字ではなくタブ文字でインデントする
    #[serde(default = "default_indent_tab")]
    pub(crate) indent_tab: bool,

    /// パーサのエラー回復機能を使うかどうか
    #[serde(default = "default_use_parser_error_recovery")]
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
        let mut config = serde_json::Map::new();

        // 設定ファイルから読み込む
        if let Some(path) = config_path {
            let file = File::open(path).map_err(|_| {
                UroboroSQLFmtError::FileNotFound("Setting file not found".to_string())
            })?;

            let reader = BufReader::new(file);

            let file_config: serde_json::Map<_, _> = serde_json::from_reader(reader)
                .map_err(|e| UroboroSQLFmtError::IllegalSettingFile(e.to_string()))?;

            config.extend(file_config);
        };

        // json文字列から読み込み、設定を上書きする
        if let Some(settings_json) = settings_json {
            let settings: serde_json::Map<_, _> =
                serde_json::from_str(settings_json).map_err(|e| {
                    UroboroSQLFmtError::Runtime(format!(
                        "Setting json is invalid. {}",
                        &e.to_string()
                    ))
                })?;

            config.extend(settings);
        }

        serde_json::from_value(serde_json::Value::Object(config))
            .map_err(|e| UroboroSQLFmtError::Runtime(e.to_string()))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            debug: default_debug(),
            tab_size: default_tab_size(),
            complement_alias: default_complement_alias(),
            trim_bind_param: default_trim_bind_param(),
            keyword_case: Case::default(),
            identifier_case: Case::default(),
            max_char_per_line: default_max_char_per_line(),
            complement_outer_keyword: default_complement_outer_keyword(),
            complement_column_as_keyword: default_complement_column_as_keyword(),
            remove_table_as_keyword: default_remove_table_as_keyword(),
            remove_redundant_nest: default_remove_redundant_nest(),
            complement_sql_id: default_complement_sql_id(),
            convert_double_colon_cast: default_convert_double_colon_cast(),
            unify_not_equal: default_unify_not_equal(),
            indent_tab: default_indent_tab(),
            use_parser_error_recovery: default_use_parser_error_recovery(),
        }
    }
}

/// 引数に与えた Config 構造体をグローバル変数 CONFIG に読み込む
pub(crate) fn load_settings(config: Config) {
    *CONFIG.write().unwrap() = config
}

/// 補完・削除を行わない設定をロードする
pub(crate) fn load_never_complement_settings() {
    let config = Config {
        debug: default_debug(),
        tab_size: default_tab_size(),
        complement_alias: false,
        trim_bind_param: false,
        keyword_case: Case::default(),
        identifier_case: Case::default(),
        max_char_per_line: default_max_char_per_line(),
        complement_sql_id: false,
        complement_outer_keyword: false,
        complement_column_as_keyword: false,
        remove_table_as_keyword: false,
        remove_redundant_nest: false,
        convert_double_colon_cast: false,
        unify_not_equal: false,
        indent_tab: true,
        use_parser_error_recovery: default_use_parser_error_recovery(),
    };

    *CONFIG.write().unwrap() = config;
}
