use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fmt::Debug;
use std::sync::Mutex;

use std::fs::File;
use std::io::BufReader;

use crate::cst::UroboroSQLFmtError;

/// 設定を保持するグローバル変数
pub(crate) static CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::new()));

/// debugのデフォルト値
fn default_debug() -> bool {
    false
}

/// tab_sizeのデフォルト値
fn default_tab_size() -> usize {
    4
}

/// complement_asのデフォルト値
fn default_complement_as() -> bool {
    true
}

/// trim_bind_paramのデフォルト値
fn default_trim_bind_param() -> bool {
    false
}

/// 設定を保持する構造体
#[derive(Deserialize, Debug, Clone, Copy)]
pub(crate) struct Config {
    /// デバッグモード
    #[serde(default = "default_debug")]
    pub(crate) debug: bool,
    /// タブ幅
    #[serde(default = "default_tab_size")]
    pub(crate) tab_size: usize,
    /// AS句がない場合に自動的に補完する
    #[serde(default = "default_complement_as")]
    pub(crate) complement_as: bool,
    /// バインド変数の中身をトリムする
    #[serde(default = "default_trim_bind_param")]
    pub(crate) trim_bind_param: bool,
}

impl Config {
    // デフォルト値で新規作成
    pub(crate) fn new() -> Config {
        // デフォルト値
        Config {
            debug: default_debug(),
            tab_size: default_tab_size(),
            complement_as: default_complement_as(),
            trim_bind_param: default_trim_bind_param(),
        }
    }
}

/// 設定ファイルの読み込み
pub(crate) fn load_settings(path: &str) -> Result<(), UroboroSQLFmtError> {
    let file = File::open(path)
        .map_err(|_| UroboroSQLFmtError::FileNotFoundError("Setting file not found".to_string()))?;

    let reader = BufReader::new(file);

    let config = serde_json::from_reader(reader)
        .map_err(|e| UroboroSQLFmtError::IllegalSettingFileError(e.to_string()))?;

    *CONFIG.lock().unwrap() = config;

    Ok(())
}
