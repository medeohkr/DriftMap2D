use gloo_net::http::Request;
use proteus_core::{BoxFuture, FetchError, TileFetcher};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "getPreloadedTile")]
    fn get_preloaded_tile(url: &str) -> Option<Vec<u8>>;
}

pub struct GlooFetcher;

impl TileFetcher for GlooFetcher {
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async move {
            let response = Request::get(url)
                .send()
                .await
                .map_err(|e| FetchError::Network(e.to_string()))?;

            if !response.ok() {
                return Err(FetchError::Http(response.status()));
            }

            response
                .binary()
                .await
                .map_err(|e| FetchError::Network(e.to_string()))
        })
    }

    fn preloaded(&self, url: &str) -> Option<Vec<u8>> {
        get_preloaded_tile(url)
    }
}