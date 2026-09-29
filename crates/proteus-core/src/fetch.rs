use std::future::Future;
use std::pin::Pin;

/// A boxed, non-`Send` future that borrows for `'a` and yields `T`.
///
/// We use this instead of `async fn` in traits because:
/// - The concrete future type of an `async fn` is unnamed, so a trait method
///   can't return it directly.
/// - `async-trait`'s default `Send` bound would break on WASM, where
///   `gloo-net`'s futures are `!Send` (they hold JS closures).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Async byte fetcher. Implementations live in the binding crates
/// (`proteus-wasm` and `proteus-cli`).
///
/// The trait deliberately does **not** require `Send`: the browser has one
/// thread, and the CLI awaits everything inline without spawning.
pub trait TileFetcher {
    /// Fetch the raw bytes at `url`.
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>>;

    /// Optional synchronous preload hook. The WASM fetcher overrides this to
    /// pull from a JS-side cache; the native fetcher inherits the default
    /// (`None`) and always goes to the network.
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