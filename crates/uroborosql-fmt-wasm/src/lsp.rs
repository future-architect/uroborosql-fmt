use futures::TryStreamExt;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::stream::JsStream;

#[wasm_bindgen]
pub struct ServerConfig {
    into_server: js_sys::AsyncIterator,
    from_server: web_sys::WritableStream,
}

#[wasm_bindgen]
impl ServerConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(into_server: js_sys::AsyncIterator, from_server: web_sys::WritableStream) -> Self {
        Self {
            into_server,
            from_server,
        }
    }
}

/// start a runtime-agnostic tower-lsp server on wasm
/// `into_server`: AsyncIterator<Uint8Array>
/// `from_server`: WritableStream<Uint8Array>
#[wasm_bindgen]
pub async fn serve(config: ServerConfig) -> Result<(), JsValue> {
    // 入力: AsyncIterator<Uint8Array>
    let input = JsStream::from(config.into_server)
        .map_ok(|value| {
            value
                .dyn_into::<js_sys::Uint8Array>()
                .expect("stream item must be Uint8Array")
                .to_vec()
        })
        .map_err(|_err| std::io::Error::from(std::io::ErrorKind::Other))
        .into_async_read();

    // output: WritableStream<Uint8Array>
    let raw =
        JsCast::unchecked_into::<wasm_streams::writable::sys::WritableStream>(config.from_server);
    let output = wasm_streams::WritableStream::from_raw(raw)
        .try_into_async_write()
        .map_err(|err| err.0)?;

    let (service, socket) = uroborosql_language_server::build_service();
    tower_lsp_server::Server::new(input, output, socket)
        .serve(service)
        .await;

    Ok(())
}
