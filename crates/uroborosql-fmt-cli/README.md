# uroborosql-fmt-cli

## 使い方

```bash
cargo r -p uroborosql-fmt-cli -- [OPTIONS] [INPUT]
```

### 引数

* `INPUT` ― フォーマット対象の SQL ファイルパス。
  * 省略した場合は STDIN から読み込む。

### オプション

| オプション                | 説明                                    |
|---------------------------|-----------------------------------------------------------------------------------------|
| `-w`, `--write`           | フォーマット結果で `INPUT` ファイルを上書きする。`--check` と同時指定不可。            |
| `--check`                 | フォーマット差分を検出した場合に終了コード 4 を返す。`--write` と同時指定不可。      |
| `--config <FILE>`         | 設定ファイルのパスを指定する。既定値はカレントディレクトリの `.uroborosqlfmtrc.json`。  |
| `-h`, `--help`            | ヘルプを表示する。                                                                     |
| `-V`, `--version`         | バージョン情報を表示する。                                                             |

### 終了コード

| 値 | 定数名        | 意味                                                    |
|----|---------------|---------------------------------------------------------|
| 0  | `Ok`          | 正常終了                                                |
| 1  | `ParseError`  | SQL の解析に失敗                                         |
| 2  | `OtherError`  | その他のエラー（無効な設定ファイル、オプション競合など） |
| 3  | `IoError`     | 入出力エラー（ファイルが見つからない、書き込み失敗等）   |
| 4  | `Diff`        | `--check` 実行時にフォーマット差分を検出                |

## 使用例

```bash
# ファイルをフォーマットして標準出力へ
uroborosql-fmt query.sql

# STDIN から読み込んでフォーマット
cat query.sql | uroborosql-fmt
uroborosql-fmt < query.sql

# フォーマット差分のみをチェック
uroborosql-fmt --check query.sql

# フォーマットしてファイルを上書き
uroborosql-fmt -w query.sql

# 設定ファイルを指定してフォーマット
uroborosql-fmt --config mycfg.json query.sql
```
