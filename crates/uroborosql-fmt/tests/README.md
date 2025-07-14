# uroborosql-fmt テスト構成

このディレクトリには、uroborosql-fmtのテストコードが含まれています。テストは主に以下の2つのカテゴリに分かれています。一方はパーサ移行タスクのために用意されたテストのため、今後なんらかの形で統合される予定です。

## テストの種類

### 1. test_all.rs

テストケース名：`test_all`

`testfiles/` ディレクトリ内のSQLファイルを対象にテストを実行します。詳しい使い方は[testfiles/readme.md](../testfiles/readme.md)を参照してください。

```bash
# 実行方法
cargo test test_all
```
```bash
# もしくは
cargo test -- --exact test_all
```

### 2. normal_cases.rs

テストケース名：`test_normal_cases`

パーサ移行用のために用意されたテストで、`test_normal_cases`ディレクトリ内のSQLファイルを対象にテストを実行します。

```bash
# 基本的な実行方法
cargo test -- --exact test_normal_cases
```

```bash
# 各ケースの実行結果を確認する場合
cargo test -- --exact test_normal_cases --nocapture
```

#### テストオプション

以下のオプションを使用してテストの動作をカスタマイズできます：

```bash
# fail-fast mode
cargo test -- --exact test_normal_cases -- --fail-fast

# sort desc mode
cargo test -- --exact test_normal_cases -- --sort-descending

# オプションの組み合わせ
cargo test -- --exact test_normal_cases -- --fail-fast --sort-descending
```

- `--fail-fast`: エラーや失敗が発生した時点でテストを中止します。デバッグ時に便利です。
- `--sort-descending`: テストケースを名前の降順（ZからA）でソートして実行します。

## ユーティリティ

### util.rs

テスト結果の差分表示などの共通ユーティリティ関数を提供します。主に以下の機能があります：

- `print_diff`: 期待値と実際の結果の差分を色付きで表示するユーティリティ関数
