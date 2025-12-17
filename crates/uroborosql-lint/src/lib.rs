mod config;
mod context;
mod diagnostic;
mod linter;
mod rule;
mod rules;
mod tree;

pub use diagnostic::{Diagnostic, Severity, SqlSpan};
pub use linter::{LintError, LintOptions, Linter};

// downward search
// - in: 対象ファイル or ディレクトリのパス
// - out: 対象ファイルパスのリスト, 設定ファイルが追加された状態の config store
// - 目的： 対象ファイルの発見・設定値の特定
//   1. 対象ファイルの洗い出し
//      - ignore クレートを使い walk する
//      - lint 対象の拡張子のみ（*.sql）
//   2. config の特定・ロード
//      - 対象ファイルの先祖ディレクトリを HashSet にして、その各パスに config が存在するか見る・あればロードする
//      - ロードした config を ConfigStore に追加する
//   3. ignore に相当するファイルを除外する
//      - ConfigStore から
