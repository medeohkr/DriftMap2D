use std::future::Future;
use std::pin::Pin;

/// A boxed, non-`Send` future that borrows for `'a` and yields `T`.
///
/// erases transport-specific future types without requiring `Send`, since
/// browser futures hold JS closures. the forcing trait uses static dispatch
/// and can use native async methods without boxing.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Async byte fetcher. Implementations live in the binding crates
/// (`proteus-wasm` and `proteus-cli`).
///
/// The trait deliberately does **not** require `Send`: the browser has one
/// thread, and native background tasks may run on a local executor.
pub trait TileFetcher {
    /// Fetch the raw bytes at `url`.
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>>;

    /// optional synchronous lookup used by the shared Rust preload cache
    fn preloaded(&self, _url: &str) -> Option<Vec<u8>> {
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("network error: {0}")]
    Network(String),
    #[error("HTTP {0}")]
    Http(u16),
}
