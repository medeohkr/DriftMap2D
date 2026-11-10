//! fetch.rs shares demand downloads with the Rust background preloader

use proteus_core::{BoxFuture, FetchError, TileFetcher};

pub struct GlooFetcher;

impl TileFetcher for GlooFetcher {
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(crate::preloader::fetch(url))
    }

    fn preloaded(&self, url: &str) -> Option<Vec<u8>> {
        crate::preloader::cached(url)
    }
}

/// raw browser transport used beneath the shared preload cache
#[derive(Clone)]
pub(crate) struct GlooTransport;

impl TileFetcher for GlooTransport {
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async move {
            let response = gloo_net::http::Request::get(url)
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
}
