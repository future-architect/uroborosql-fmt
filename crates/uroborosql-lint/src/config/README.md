# uroborosql-lint config の構成について

このモジュールは lint 設定のロードと解決を担当します。
lint コアは設定を直接パースせず、解決済みの状態だけを受け取り実行します。

## モジュール構成
- `lint_config.rs`
  - JSON のワイヤ形式を定義（`LintConfigObject`）
- `types.rs`
  - ルール設定の内部型を定義（`RuleLevel`, `RuleSetting`）
- `overrides.rs`
  - override のコンパイル済み表現を定義（`ResolvedOverride`）
- `config_store.rs`
  - ローダ + リゾルバ を定義（`ConfigStore`, `ResolvedLintConfig`, `ResolvedDbConfig`）

## データフロー
1) `{root_dir}/.uroborosqllintrc.json`（または `--config`）を読み込む
2) `LintConfigObject` にデシリアライズ（ワイヤ形式）
3) 内部表現 `LintConfig` に変換:
   - ルールのレベルを `RuleSetting` に変換
   - override の glob を `ResolvedOverride` にコンパイル
   - ignore の glob をコンパイル
4) 各ファイルで `ConfigStore::resolve(file)`:
   - override を順番に適用（後勝ち）
   - ルール名を `RuleEnum` に変換
   - `ResolvedLintConfig` を返す
5) Linter は `ResolvedLintConfig` のみを使って実行する

ignore のフィルタは `resolve` の前に `ConfigStore::is_ignored` で行う。

## 構造体の関係
```
LintConfigObject (serde JSON)
  ├─ db: Option<DbConfig>
  ├─ rules: HashMap<String, Value>
  ├─ overrides: Vec<LintOverride>
  └─ ignore: Vec<String>
        |
        v
LintConfig (internal, compiled)
  ├─ rules: HashMap<String, RuleSetting>
  ├─ overrides: Vec<ResolvedOverride>
  ├─ ignore: GlobSet
  └─ db: Option<ResolvedDbConfig>
        |
        v
ResolvedLintConfig
  └─ rules: Vec<(RuleEnum, Severity)>
```

## ルール設定
- `RuleLevel`: `off | warn | error`（`0 | 1 | 2` も許可）
