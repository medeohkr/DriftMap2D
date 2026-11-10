//! preloader.rs caches background tiles and shares in-flight downloads with demand reads

use crate::{BoxFuture, FetchError, TileFetcher};
use futures_util::{
    future::{LocalBoxFuture, Shared},
    FutureExt,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

type Spawn = fn(BoxFuture<'static, ()>);
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

    fn complete(
        &mut self,
        url: String,
        result: &Result<Rc<Vec<u8>>, Rc<FetchError>>,
        loaded_ms: f64,
    ) {
        self.pending.remove(&url);
        match result {
            Ok(bytes) if self.wanted.contains(&url) || url.ends_with("/manifest.json") => {
                self.failed.remove(&url);
                self.ready.insert(
                    url,
                    Entry {
                        bytes: bytes.clone(),
                        loaded_ms,
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

/// shared byte cache with up to four background downloads on a local executor
///
/// supply a raw transport and a nonblocking task spawner for the host runtime
#[derive(Clone)]
pub struct Preloader<F> {
    state: Rc<RefCell<State>>,
    fetcher: F,
    spawn: Spawn,
    now_ms: fn() -> f64,
}

impl<F: TileFetcher + Clone + 'static> Preloader<F> {
    /// transport, task spawning, and clock are supplied by the binding crate
    pub fn new(fetcher: F, spawn: Spawn, now_ms: fn() -> f64) -> Self {
        Self {
            state: Rc::new(RefCell::new(State::default())),
            fetcher,
            spawn,
            now_ms,
        }
    }

    /// issue a revision before awaiting metadata so stale plans cannot win
    pub fn begin_plan(&self) -> u64 {
        let mut state = self.state.borrow_mut();
        state.revision = state.revision.wrapping_add(1);
        state.revision
    }

    pub fn finish_plan(&self, revision: u64, urls: Vec<String>) {
        if revision == self.state.borrow().revision {
            self.plan(urls);
        }
    }

    pub fn cached(&self, url: &str) -> Option<Rc<Vec<u8>>> {
        let mut state = self.state.borrow_mut();
        if url.ends_with("/manifest.json")
            && state
                .ready
                .get(url)
                .is_some_and(|entry| (self.now_ms)() - entry.loaded_ms > METADATA_TTL_MS)
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
        let fetcher = self.fetcher.clone();
        let now_ms = self.now_ms;
        let pending = async move {
            let result = fetcher
                .fetch_bytes(&key)
                .await
                .map(Rc::new)
                .map_err(Rc::new);
            if let Some(state) = weak.upgrade() {
                state.borrow_mut().complete(key, &result, now_ms());
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
            (self.spawn)(Box::pin(async move {
                if let Err(error) = pending.await {
                    log::warn!("forcing preload failed {url}: {error}");
                }
                loader.state.borrow_mut().workers -= 1;
                loader.pump();
            }));
        }
    }

    fn plan(&self, urls: Vec<String>) {
        self.state.borrow_mut().plan(urls);
        self.pump();
    }
}

impl<F: TileFetcher + Clone + 'static> TileFetcher for Preloader<F> {
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        let pending = self.request(url);
        Box::pin(async move {
            pending
                .await
                .map(|bytes| (*bytes).clone())
                .map_err(|error| match &*error {
                    FetchError::Network(message) => FetchError::Network(message.clone()),
                    FetchError::Http(code) => FetchError::Http(*code),
                })
        })
    }

    fn preloaded(&self, url: &str) -> Option<Vec<u8>> {
        self.cached(url).map(|bytes| (*bytes).clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Clone)]
    struct Mock(fn(String) -> LocalBoxFuture<'static, Result<Vec<u8>, FetchError>>);
    impl TileFetcher for Mock {
        fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
            (self.0)(url.to_owned())
        }
    }
    fn spawn(task: BoxFuture<'static, ()>) {
        tokio::task::spawn_local(task);
    }
    fn now_ms() -> f64 {
        1_000_000.0
    }
    fn loader(
        download: fn(String) -> LocalBoxFuture<'static, Result<Vec<u8>, FetchError>>,
    ) -> Preloader<Mock> {
        Preloader::new(Mock(download), spawn, now_ms)
    }

    thread_local! { static REQUESTS: Cell<usize> = const { Cell::new(0) }; }

    #[test]
    fn older_marker_or_simulation_plans_cannot_replace_a_newer_plan() {
        let loader = loader(mock);
        let stale = loader.begin_plan();
        let current = loader.begin_plan();
        loader.finish_plan(current, Vec::new());
        loader.finish_plan(stale, vec!["stale tile".into()]);
        assert!(loader.state.borrow().wanted.is_empty());
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
        let loader = loader(mock);
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
        let loader = loader(mock);
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
        let loader = loader(fail_once);
        loader.state.borrow_mut().plan(vec!["tile".into()]);
        assert!(loader.request("tile").await.is_err());
        assert!(loader.state.borrow().failed.contains("tile"));
        assert_eq!(*loader.request("tile").await.unwrap(), [4]);
        assert!(!loader.state.borrow().failed.contains("tile"));
    }

    #[test]
    fn changing_plans_prunes_tiles_and_metadata_expires() {
        let loader = loader(mock);
        let bytes = Rc::new(vec![1]);
        let mut state = loader.state.borrow_mut();
        state.plan(vec!["old".into()]);
        state.complete("old".into(), &Ok(bytes.clone()), now_ms());
        state.complete("root/manifest.json".into(), &Ok(bytes), now_ms());
        state.plan(vec!["new".into()]);
        assert!(!state.ready.contains_key("old"));
        assert!(state.ready.contains_key("root/manifest.json"));
        state.ready.get_mut("root/manifest.json").unwrap().loaded_ms -= METADATA_TTL_MS + 1.0;
        drop(state);
        assert!(loader.cached("root/manifest.json").is_none());
    }

    #[tokio::test]
    async fn background_workers_are_bounded_and_share_inflight_demand() {
        thread_local! {
            static ACTIVE: Cell<usize> = const { Cell::new(0) };
            static PEAK: Cell<usize> = const { Cell::new(0) };
        }
        fn slow(_: String) -> LocalBoxFuture<'static, Result<Vec<u8>, FetchError>> {
            async {
                REQUESTS.with(|n| n.set(n.get() + 1));
                ACTIVE.with(|n| {
                    n.set(n.get() + 1);
                    PEAK.with(|peak| peak.set(peak.get().max(n.get())));
                });
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                ACTIVE.with(|n| n.set(n.get() - 1));
                Ok(vec![9])
            }
            .boxed_local()
        }
        tokio::task::LocalSet::new()
            .run_until(async {
                REQUESTS.with(|n| n.set(0));
                ACTIVE.with(|n| n.set(0));
                PEAK.with(|n| n.set(0));
                let loader = loader(slow);
                loader.finish_plan(
                    loader.begin_plan(),
                    (0..10).map(|i| format!("tile{i}")).collect(),
                );
                assert_eq!(loader.state.borrow().workers, MAX_BACKGROUND);
                let url = loader.state.borrow().pending.keys().next().unwrap().clone();
                assert_eq!(loader.fetch_bytes(&url).await.unwrap(), [9]);
                tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    while loader.state.borrow().workers != 0 {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert_eq!(REQUESTS.with(Cell::get), 10);
                assert_eq!(PEAK.with(Cell::get), MAX_BACKGROUND);
                assert_eq!(loader.state.borrow().ready.len(), 10);
                assert!(loader.state.borrow().pending.is_empty());
            })
            .await;
    }

    #[tokio::test]
    async fn failed_background_work_is_suppressed_until_demand_retries() {
        fn fail_once(_: String) -> LocalBoxFuture<'static, Result<Vec<u8>, FetchError>> {
            async {
                REQUESTS.with(|n| {
                    let old = n.replace(n.get() + 1);
                    if old == 0 {
                        Err(FetchError::Http(503))
                    } else {
                        Ok(vec![7])
                    }
                })
            }
            .boxed_local()
        }
        tokio::task::LocalSet::new()
            .run_until(async {
                REQUESTS.with(|n| n.set(0));
                let loader = loader(fail_once);
                loader.finish_plan(loader.begin_plan(), vec!["tile".into()]);
                tokio::task::yield_now().await;
                assert_eq!(loader.state.borrow().workers, 0);
                assert_eq!(REQUESTS.with(Cell::get), 1);
                loader.finish_plan(loader.begin_plan(), vec!["tile".into()]);
                assert_eq!(loader.state.borrow().workers, 0);
                assert_eq!(loader.fetch_bytes("tile").await.unwrap(), [7]);
                assert_eq!(REQUESTS.with(Cell::get), 2);
            })
            .await;
    }

    #[tokio::test]
    async fn replacing_or_clearing_a_plan_drops_stale_results_and_starts_new_work() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let loader = loader(mock);
                loader.finish_plan(loader.begin_plan(), vec!["old".into()]);
                loader.finish_plan(loader.begin_plan(), vec!["new".into()]);
                tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    while loader.state.borrow().workers != 0 {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(loader.cached("old").is_none());
                assert!(loader.cached("new").is_some());
                loader.finish_plan(loader.begin_plan(), vec!["discard".into()]);
                loader.finish_plan(loader.begin_plan(), Vec::new());
                tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    while loader.state.borrow().workers != 0 {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(loader.state.borrow().ready.is_empty());
            })
            .await;
    }
}
