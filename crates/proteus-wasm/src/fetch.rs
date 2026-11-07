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
