use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fs::File;
use std::io::BufReader;
use std::sync::RwLock;

use crate::error::UroboroSQLFmtError;

/// 設定を保持するグローバル変数
pub(crate) static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| RwLock::new(Config::new()));

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
fn default_complement_as_keyword() -> bool {
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Case {
    Upper,
    Lower,
    Preserve,
}

impl Default for Case {
    /// Caseのデフォルト値(upper)
    fn default() -> Self {
        Case::Upper
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
pub(crate) struct Config {
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
    #[serde(default = "default_complement_as_keyword")]
    pub(crate) complement_as_keyword: bool,
    /// 余分な括弧を自動で除去する
    #[serde(default = "default_remove_redundant_nest")]
    pub(crate) remove_redundant_nest: bool,
    /// /* _SQL_ID_ */がない場合に自動で補完する
    #[serde(default = "default_complement_sql_id")]
    pub(crate) complement_sql_id: bool,
}

impl Config {
    // デフォルト値で新規作成
    pub(crate) fn new() -> Config {
        // デフォルト値
        Config {
            debug: default_debug(),
            tab_size: default_tab_size(),
            complement_alias: default_complement_alias(),
            trim_bind_param: default_trim_bind_param(),
            keyword_case: Case::default(),
            identifier_case: Case::default(),
            max_char_per_line: default_max_char_per_line(),
            complement_sql_id: default_complement_sql_id(),
            complement_outer_keyword: default_complement_outer_keyword(),
            complement_as_keyword: default_complement_as_keyword(),
            remove_redundant_nest: default_remove_redundant_nest(),
        }
    }

    /// Json 文字列から Config 構造体を生成する
    pub(crate) fn from_json_str(json_str: &str) -> Result<Config, UroboroSQLFmtError> {
        serde_json::from_str(json_str).map_err(|e| {
            UroboroSQLFmtError::Runtime(format!("Setting json is invalid.{}", &e.to_string()))
        })
    }

    /// 設定ファイルのパスから Config 構造体を生成する
    pub(crate) fn from_path(path: &str) -> Result<Config, UroboroSQLFmtError> {
        let file = File::open(path)
            .map_err(|_| UroboroSQLFmtError::FileNotFound("Setting file not found".to_string()))?;

        let reader = BufReader::new(file);

        serde_json::from_reader(reader)
            .map_err(|e| UroboroSQLFmtError::IllegalSettingFile(e.to_string()))
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
        complement_as_keyword: false,
        remove_redundant_nest: false,
    };

    *CONFIG.write().unwrap() = config;
}
