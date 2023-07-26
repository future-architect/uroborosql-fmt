# wasm 化メモ

## クイックスタート

1. `python3 -m http.server 8000`
2. http://localhost:8000/wasm/index.html

## wasm 化手順

1. emsdk のインストール ([Download and install — Emscripten 3\.1\.44\-git \(dev\) documentation](https://emscripten.org/docs/getting_started/downloads.html), **ドキュメント記入時のバージョンは 3.1.15**)
2. `build.sh` の emsdk のパスを自分の環境のものに変える
3. `rustup target add wasm32-unknown-emscripten` で target を追加
4. `npm install` でパッケージをインストール
5. `npm run build-wasm` で wasm と js を生成 ( `source build.sh` でもOK)
6. `npm run serve` で正しく動作すればOK
