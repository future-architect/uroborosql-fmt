# uroborosql-fmt テスト構成

このディレクトリには、uroborosql-fmtのテストコードが含まれています。テストは主に以下の3つのカテゴリに分かれています。

## テストの種類

### 1. test_all.rs

テストケース名：`test_all`

元々存在するテストで、`testfiles`ディレクトリ内のSQLファイルを対象に、`tree-sitter`を使用したバージョンのテストを実行します。このテストは、既存の機能が正しく動作することを確認するためのものです。

```bash
# 実行方法
cargo test test_all
```
```bash
# もしくは
cargo test -- --exact test_all
```

### 2. pgcst_coverage.rs

テストケース名：`test_with_coverage`

パーサ移行用のテストで、`testfiles`ディレクトリ内のSQLファイルを対象に新パーサー(`postgresql-cst-parser`)を使用してテストを実行します。このテストは、既存のテストケースに対する新パーサーの実装カバレッジを報告するためのものです。

テスト結果が合わなくてもテストは失敗しません。新パーサーでどの程度のSQLがサポートされているかを確認するために使用します。

```bash
# 実行方法
cargo test -- --exact test_with_coverage
```

また、このテストでは出力する情報をカスタマイズできます。失敗したテストケースを表示する場合等は、テスト部分で Config を書き換えて実行してください。

```rust
// pgcst_coverage.rs
#[test]
fn test_with_coverage() {
    let results = run_test_suite();

    // before(default):
    // let config = TestReportConfig::default();

    // after:
    let mut config = TestReportConfig::default();
    config.show_failed_cases = true; // 失敗したケースを表示
    config.show_error_annotations = true; // アノテーションを表示

    print_coverage_report(&results, &config);
}
```

また、このときの結果は基本的にstdoutへと出力されます。確認する場合は`--nocapture`オプションをつけて実行してください。

```bash
# 実行方法
cargo test -- --exact test_with_coverage --nocapture
```

### 3. pgcst_normal_cases.rs

テストケース名：`test_normal_cases`

パーサ移行用のテストで、`test_normal_cases`ディレクトリ内のSQLファイルを対象に新パーサーでのテストを実行します。このテストは、段階的に機能を増やしたSQLを用意し、リグレッションを管理しながら移行を進めるために使用します。

```bash
# 実行方法
cargo test -- --exact test_normal_cases
```
```bash
# 各ケースの実行結果を確認する場合
cargo test -- --exact test_normal_cases --nocapture
```

## ユーティリティ

### pgcst_util.rs

テスト結果の差分表示などの共通ユーティリティ関数を提供します。主に以下の機能があります：

- `print_diff`: 期待値と実際の結果の差分を色付きで表示するユーティリティ関数

## テストケースの種類

### 1. testfiles

元々のフォーマッターのテストケースで、`tree-sitter`を使用して開発していた際に追加されたものです。様々なSQLパターンを網羅しています。

### 2. test_normal_cases

移行用のテストケースで、新パーサーでのサポート範囲を増やすために段階的に機能を追加していくためのものです。少しずつ置き換えていくための正常系テストとして使用します。

## 注意事項
`pgcst` というprefixがついたテストおよびファイルは、パーサ移行タスクのために用意されたものです。そのためこれらはパーサ移行の終了に伴い削除される予定です。
