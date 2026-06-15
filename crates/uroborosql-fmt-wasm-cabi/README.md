# uroborosql-fmt-wasm-cabi

Builds uroborosql-fmt as a wasm module that exposes a **minimal C-ABI**.

Unlike [`uroborosql-fmt-wasm`](../uroborosql-fmt-wasm) (wasm-bindgen) or the distributed demo (emscripten),
this crate uses **neither wasm-bindgen nor emscripten, and has no dependency on JS glue**.
All interaction happens through linear memory plus numeric (pointer/length) arguments, and the module has **zero imports**.
As a result it can be consumed as-is from non-JS hosts such as JVM WebAssembly runtimes (e.g. [Chicory](https://chicory.dev)),
without any additional glue.

## Exported functions

| Function | Purpose |
|---|---|
| `alloc(size) -> ptr` | Allocate linear memory for input |
| `dealloc(ptr, size)` | Free memory allocated by `alloc` |
| `format(src_ptr, src_len, cfg_ptr, cfg_len) -> ptr` | Format SQL and return the head of a result buffer |
| `free_result(ptr)` | Free the buffer returned by `format` |

Result buffer layout (a contract shared with the host, little-endian):

```text
offset 0 : u32  cap     ... total allocated size of the buffer (used by free_result)
offset 4 : u8   status  ... 0 = OK / 1 = error
offset 5 : u32  len     ... byte length of body
offset 9 : u8[len] body ... UTF-8. Formatted SQL on success, error message on failure
```

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo build -p uroborosql-fmt-wasm-cabi --target wasm32-unknown-unknown --release
# => target/wasm32-unknown-unknown/release/uroborosql_fmt_wasm_cabi.wasm
```

## Host-side calling sequence

1. Allocate with `alloc(len)` and write UTF-8 bytes into linear memory (the SQL, and the config JSON if any).
2. Call `format(src_ptr, src_len, cfg_ptr, cfg_len)` (pass `cfg_ptr = 0, cfg_len = 0` when there is no config).
3. Read `status` / `len` / `body` from the returned pointer.
4. Free with `free_result(ptr)`, and `dealloc(ptr, len)` for the input.

Config JSON keys follow uroborosql-fmt's
[Configuration options](../../README.md#configuration-options).
