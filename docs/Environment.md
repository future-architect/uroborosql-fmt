# uroborosql-fmt


## 環境構築

### 準備

Rust
* https://gitlab.nasa.future.co.jp/oss-incubate/rust-sql-formatter/rustup-win
* gnuに変更するよう注意

GCC(必要かどうか不明、必要に応じて)
* https://gitlab.nasa.future.co.jp/oss-incubate/rust-sql-formatter/rustup-win/-/blob/main/Related-to-MinGW.md 
* 解凍に7-Zip(https://sevenzip.osdn.jp/ )が必要(齋藤のPCにはなかったはず)

npm(必要に応じて)
* https://palette-doc.rtfa.as/v6/latest/front-end/1.settings/#installs
* gitの設定も上記URLにある
* 齋藤はv16(Node.js 16.17.0)をインストールした
* npmのプロキシ設定について、VPN環境下ではユーザ名、パスワードが必要な点に注意

```
npm -g config set proxy http://<user>:<password>@proxy.future.co.jp:8000
npm -g config set https-proxy http://<user>:<password>@proxy.future.co.jp:8000
npm -g config set noproxy localhost,.future.co.jp
npm -g config set strict-ssl false
```


### uroborosql-fmtの開発環境

現在、GitLabにcloneしたtree-sitter-sqlを使用している。
そのため、ビルドするためには `.\.cargo\config` に次を加える必要がある。

```
[net]
git-fetch-with-cli = true
```

また、tree-sitter-sqlのclone時にパスが長くてうまくいかない場合は、次のコマンドで解決できる。

```
git config --global core.longpaths true
```

### tree-sitter-cli

tree-sitter-sqlの文法の変更では、パーサを再生成する際にtree-sitter-cliが必要になる。

`npm install` ではプロキシの関係上インストールが難しく、`cargo install` ではVisual Studioを前提としているため正常に動作しない。
そこで、`git clone` でソースコードをダウンロードして、コードを修正して対応する。
以下の手順を実行する。

1. tree-sitterをcloneする
2. `cli\loader\src\lib.rs` の376行目の条件式に `false &&` を追加する
3. tree-sitterのルートディレクトリで `cargo install --path cli` を実行してインストールできる

`npm test` または `tree-sitter test` が通れば成功。

### Rust to WebAssemblyのチュートリアルを動くようにする

https://developer.mozilla.org/ja/docs/WebAssembly/Rust_to_wasm
のチュートリアルに従って、RustをWebAssemblyで動かす。

ビルドのために `wasm-pack` が必要とされている(uroborosql-fmtのビルドは難しいかもしれない)。
これは `cargo install` でインストールできる。齋藤の環境では、`--no-default-feature` オプションを付けないとエラーが発生した(参考: https://qiita.com/t_katsumura/items/526bd21442e3c4bf2f8b)。

(VPN環境では、環境変数のhttps_proxyにユーザ名とパスワードを加える必要がある)

```
cargo install wasm-pack --no-default-feature
```

<!-- RustからJavaScriptの関数を呼び出すときや、Rustの関数をJavaScriptから呼び出すときは、関数に `#[wasm-binding]` を付与する。 -->

ビルドの前に、`Cargo.toml` に次を加える。

```
[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
```

チュートリアルのプロジェクトは、`wasm-pack build -- target (web | bundler)` でビルドできる。


### uroborosql-fmtのコンパイルに向けて

uroborosql-fmtを `wasm-pack build` でビルドしようとすると、tree-sitterのコンパイルで失敗する。

代替案
* Emscripten(実験中)
* wasi-sdk

参考:https://zenn.dev/newgyu/articles/4240df5d2a7d55

#### Emscripten

https://emscripten.org/docs/getting_started/downloads.html
に従ってインストールする。
pythonスクリプトで書かれているため、インストールにはpythonが必要。
pythonは
https://www.python.org/downloads/
でダウンロードできる。
しかし、インターンで貸与されたPCでは、(中身のない?)`python` がすでに存在していたため、pythonをインストールしてパスを追加するだけでは動作しなかった。
Windows Powershellでは、次のコマンドでパスを確認できる。

```
$ gcm python | fl


Name            : python.exe
CommandType     : Application
Definition      : C:\Users\[username]\AppData\Local\Microsoft\WindowsApps\python.exe
Extension       : .exe
Path            : C:\Users\[username]\AppData\Local\Microsoft\WindowsApps\python.exe
FileVersionInfo : File: ...
```

ユーザの環境変数から以下を削除することでとりあえずは動作した。
```
%USERPROFILE%\AppData\Local\Microsoft\WindowsApps
```

現在実験中...
参考: https://zenn.dev/newgyu/articles/8bff73505c7b35