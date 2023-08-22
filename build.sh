# emccを有効にする
source ../emsdk/emsdk_env.sh

# emccの設定変更
export EMCC_CFLAGS="-O3"
# tree-sitter-sqlのビルドを実行
cargo build --package tree-sitter-sql --target wasm32-unknown-emscripten --release

# emccの設定変更
export EMCC_CFLAGS="-O3 
                    -o ./wasm/uroborosql-fmt.js
                    -s ALLOW_MEMORY_GROWTH=1
                    -s STACK_SIZE=5MB
                    -s EXPORTED_FUNCTIONS=['_format_sql','_free_format_string'] 
                    -s EXPORTED_RUNTIME_METHODS=ccall"
# 全体のビルドを実行
cargo build --package uroborosql-fmt-wasm --target wasm32-unknown-emscripten --release
