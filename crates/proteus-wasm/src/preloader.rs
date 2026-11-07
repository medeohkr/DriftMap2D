//! preloader.rs caches background tiles and shares in-flight downloads with demand reads

use futures_util::{
    future::{LocalBoxFuture, Shared},
    FutureExt,
};
use gloo_net::http::Request;
use proteus_core::FetchError;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

type Download = fn(String) -> LocalBoxFuture<'static, Result<Vec<u8>, FetchError>>;
type Pending = Shared<LocalBoxFuture<'static, Result<Rc<Vec<u8>>, Rc<FetchError>>>>;
const MAX_BACKGROUND: usize = 4;
const METADATA_TTL_MS: f64 = 300_000.0;

struct Entry {
    bytes: Rc<Vec<u8>>,
    loaded_ms: f64,
}

#[derive(Default)]
struct State {
    ready: HashMap<String, Entry>,
    pending: HashMap<String, Pending>,
    wanted: HashSet<String>,
    failed: HashSet<String>,
    workers: usize,
    revision: u64,
}

impl State {
    fn plan(&mut self, urls: Vec<String>) {
        self.revision = self.revision.wrapping_add(1);
        let wanted: HashSet<_> = urls.into_iter().collect();
        if wanted != self.wanted {
            self.failed.clear();
        }
        self.wanted = wanted;
        self.ready
            .retain(|url, _| self.wanted.contains(url) || url.ends_with("/manifest.json"));
    }

    fn complete(&mut self, url: String, result: &Result<Rc<Vec<u8>>, Rc<FetchError>>) {
        self.pending.remove(&url);
        match result {
            Ok(bytes) if self.wanted.contains(&url) || url.ends_with("/manifest.json") => {
                self.failed.remove(&url);
                self.ready.insert(
                    url,
                    Entry {
                        bytes: bytes.clone(),
                        loaded_ms: now_ms(),
                    },
                );
            }
            Err(_) if self.wanted.contains(&url) => {
                self.failed.insert(url);
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct Preloader {
    state: Rc<RefCell<State>>,
    download: Download,
}

impl Preloader {
    fn new(download: Download) -> Self {
        Self {
            state: Rc::new(RefCell::new(State::default())),
            download,
        }
    }

    fn cached(&self, url: &str) -> Option<Rc<Vec<u8>>> {
        let mut state = self.state.borrow_mut();
        if url.ends_with("/manifest.json")
            && state
                .ready
                .get(url)
                .is_some_and(|entry| now_ms() - entry.loaded_ms > METADATA_TTL_MS)
        {
            state.ready.remove(url);
        }
        state.ready.get(url).map(|entry| entry.bytes.clone())
    }

    fn request(&self, url: &str) -> Pending {
        if let Some(bytes) = self.cached(url) {
            return futures_util::future::ready(Ok(bytes))
                .boxed_local()
                .shared();
        }
        if let Some(pending) = self.state.borrow().pending.get(url) {
            return pending.clone();
        }
        let key = url.to_owned();
        let weak = Rc::downgrade(&self.state);
        let download = self.download;
        let pending = async move {
            let result = download(key.clone()).await.map(Rc::new).map_err(Rc::new);
            if let Some(state) = weak.upgrade() {
                state.borrow_mut().complete(key, &result);
            }
            result
        }
        .boxed_local()
        .shared();
        self.state
            .borrow_mut()
            .pending
            .insert(url.to_owned(), pending.clone());
        pending
    }

    fn pump(&self) {
        loop {
            let next = {
                let state = self.state.borrow();
                if state.workers >= MAX_BACKGROUND {
                    return;
                }
                state
                    .wanted
                    .iter()
                    .find(|url| {
                        !state.ready.contains_key(*url)
                            && !state.pending.contains_key(*url)
                            && !state.failed.contains(*url)
                    })
                    .cloned()
            };
            let Some(url) = next else {
                return;
            };
            let pending = self.request(&url);
            self.state.borrow_mut().workers += 1;
            let loader = self.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = pending.await {
                    log::warn!("forcing preload failed {url}: {error}");
                }
                loader.state.borrow_mut().workers -= 1;
                loader.pump();
            });
        }
    }

    fn plan(&self, urls: Vec<String>) {
        self.state.borrow_mut().plan(urls);
        self.pump();
    }
}

fn download(url: String) -> LocalBoxFuture<'static, Result<Vec<u8>, FetchError>> {
    async move {
        let response = Request::get(&url)
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
    }
    .boxed_local()
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

thread_local! { static LOADER: Preloader = Preloader::new(download); }

pub(crate) fn cached(url: &str) -> Option<Vec<u8>> {
    LOADER.with(|loader| loader.cached(url).map(|bytes| (*bytes).clone()))
}

pub(crate) async fn fetch(url: &str) -> Result<Vec<u8>, FetchError> {
    let pending = LOADER.with(|loader| loader.request(url));
    pending
        .await
        .map(|bytes| (*bytes).clone())
        .map_err(|error| match &*error {
            FetchError::Network(message) => FetchError::Network(message.clone()),
            FetchError::Http(code) => FetchError::Http(*code),
        })
}

/// invalidate older marker plans before awaiting their metadata request
pub(crate) fn begin_plan() -> u64 {
    LOADER.with(|loader| {
        let mut state = loader.state.borrow_mut();
        state.revision = state.revision.wrapping_add(1);
        state.revision
    })
}

pub(crate) fn finish_plan(revision: u64, urls: Vec<String>) {
    LOADER.with(|loader| {
        let current = loader.state.borrow().revision;
        if revision == current {
            loader.plan(urls);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! { static REQUESTS: Cell<usize> = const { Cell::new(0) }; }

    #[test]
    fn older_marker_or_simulation_plans_cannot_replace_a_newer_plan() {
        let stale = begin_plan();
        let current = begin_plan();
        finish_plan(current, Vec::new());
        finish_plan(stale, vec!["stale tile".into()]);
        LOADER.with(|loader| assert!(loader.state.borrow().wanted.is_empty()));
    }
    fn mock(_: String) -> LocalBoxFuture<'static, Result<Vec<u8>, FetchError>> {
        async {
            REQUESTS.with(|count| count.set(count.get() + 1));
            tokio::task::yield_now().await;
            Ok(vec![1, 2, 3])
        }
        .boxed_local()
    }

    #[tokio::test]
    async fn demand_and_prefetch_share_one_request_and_cache_the_result() {
        REQUESTS.with(|count| count.set(0));
        let loader = Preloader::new(mock);
        loader.state.borrow_mut().plan(vec!["tile".into()]);
        let prefetch = loader.request("tile");
        let demand = loader.request("tile");
        let (a, b) = tokio::join!(prefetch, demand);
        assert!(Rc::ptr_eq(&a.unwrap(), &b.unwrap()));
        assert_eq!(REQUESTS.with(Cell::get), 1);
        assert!(loader.state.borrow().pending.is_empty());
        assert_eq!(*loader.request("tile").await.unwrap(), [1, 2, 3]);
        assert_eq!(REQUESTS.with(Cell::get), 1);
    }

    #[tokio::test]
    async fn stale_completions_do_not_repopulate_the_cache() {
        let loader = Preloader::new(mock);
        loader.state.borrow_mut().plan(vec!["old".into()]);
        let pending = loader.request("old");
        loader.state.borrow_mut().plan(vec!["new".into()]);
        pending.await.unwrap();
        assert!(loader.cached("old").is_none());
        assert!(loader.state.borrow().pending.is_empty());
    }

    #[tokio::test]
    async fn failed_prefetches_can_be_retried_on_demand() {
        fn fail_once(_: String) -> LocalBoxFuture<'static, Result<Vec<u8>, FetchError>> {
            async {
                REQUESTS.with(|count| {
                    let n = count.replace(count.get() + 1);
                    if n == 0 {
                        Err(FetchError::Http(503))
                    } else {
                        Ok(vec![4])
                    }
                })
            }
            .boxed_local()
        }
        REQUESTS.with(|count| count.set(0));
        let loader = Preloader::new(fail_once);
        loader.state.borrow_mut().plan(vec!["tile".into()]);
        assert!(loader.request("tile").await.is_err());
        assert!(loader.state.borrow().failed.contains("tile"));
        assert_eq!(*loader.request("tile").await.unwrap(), [4]);
        assert!(!loader.state.borrow().failed.contains("tile"));
    }

    #[test]
    fn changing_plans_prunes_tiles_and_metadata_expires() {
        let loader = Preloader::new(mock);
        let bytes = Rc::new(vec![1]);
        let mut state = loader.state.borrow_mut();
        state.plan(vec!["old".into()]);
        state.complete("old".into(), &Ok(bytes.clone()));
        state.complete("root/manifest.json".into(), &Ok(bytes));
        state.plan(vec!["new".into()]);
        assert!(!state.ready.contains_key("old"));
        assert!(state.ready.contains_key("root/manifest.json"));
        state.ready.get_mut("root/manifest.json").unwrap().loaded_ms -= METADATA_TTL_MS + 1.0;
        drop(state);
        assert!(loader.cached("root/manifest.json").is_none());
    }
}
