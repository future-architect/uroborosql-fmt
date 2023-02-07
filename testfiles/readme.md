# uroborosql-fmt/testfiles

以下のコマンドでフォーマッタのテストを行う。
```console
cargo test
```

テストファイルは`./testfiles/src/`下に置く。
テストが実行されたら、`./testfiles/src/`下にあるすべての`.sql`に対してフォーマットを行い、`./testfiles/dst/`の対応するパスにフォーマット後の`.sql`ファイルが生成される。

テストの追加・変更・移動を行う際には、`./testfiles/src/`を変更すれば、テスト実行時に自動的に`./testfiles/dst/`ディレクトリも変更される。