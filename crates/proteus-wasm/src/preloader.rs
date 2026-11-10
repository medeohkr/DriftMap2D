//! preloader.rs connects the shared cache to the browser executor and clock

use crate::fetch::GlooTransport;
use proteus_core::{BoxFuture, FetchError, Preloader, TileFetcher};

fn spawn(task: BoxFuture<'static, ()>) {
    wasm_bindgen_futures::spawn_local(task);
}

fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            * 1000.0
    }
}

thread_local! { static LOADER: Preloader<GlooTransport> = Preloader::new(GlooTransport, spawn, now_ms); }

pub(crate) fn cached(url: &str) -> Option<Vec<u8>> {
    LOADER.with(|loader| loader.preloaded(url))
}

pub(crate) async fn fetch(url: &str) -> Result<Vec<u8>, FetchError> {
    let loader = LOADER.with(Clone::clone);
    loader.fetch_bytes(url).await
}

pub(crate) fn begin_plan() -> u64 {
    LOADER.with(|loader| loader.begin_plan())
}

pub(crate) fn finish_plan(revision: u64, urls: Vec<String>) {
    LOADER.with(|loader| loader.finish_plan(revision, urls));
}
