use proteus_core::{BoxFuture, FetchError, Preloader, TileFetcher};
use url::Url;

#[derive(Clone)]
struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl TileFetcher for ReqwestTransport {
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        // file:// URLs read from disk. Everything else goes over HTTP.
        if url.starts_with("file://") {
            return Box::pin(async move {
                let parsed = Url::parse(url)
                    .map_err(|e| FetchError::Network(format!("invalid file URL: {e}")))?;
                let path = parsed
                    .to_file_path()
                    .map_err(|_| FetchError::Network(format!("not a valid file path: {url}")))?;
                tokio::fs::read(&path).await.map_err(|e| {
                    FetchError::Network(format!("file read failed: {}: {e}", path.display()))
                })
            });
        }

        Box::pin(async move {
            let resp = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| FetchError::Network(e.to_string()))?;

            if !resp.status().is_success() {
                return Err(FetchError::Http(resp.status().as_u16()));
            }

            Ok(resp
                .bytes()
                .await
                .map_err(|e| FetchError::Network(e.to_string()))?
                .to_vec())
        })
    }
}

/// native demand reads and background requests share the same core cache
#[derive(Clone)]
pub struct ReqwestFetcher {
    preloader: Preloader<ReqwestTransport>,
}

fn spawn(task: BoxFuture<'static, ()>) {
    tokio::task::spawn_local(task);
}
fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        * 1000.0
}

impl ReqwestFetcher {
    pub fn new() -> Self {
        Self {
            preloader: Preloader::new(ReqwestTransport::new(), spawn, now_ms),
        }
    }
    pub fn begin_plan(&self) -> u64 {
        self.preloader.begin_plan()
    }
    pub fn finish_plan(&self, revision: u64, urls: Vec<String>) {
        self.preloader.finish_plan(revision, urls);
    }
}

impl TileFetcher for ReqwestFetcher {
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        self.preloader.fetch_bytes(url)
    }
    fn preloaded(&self, url: &str) -> Option<Vec<u8>> {
        self.preloader.preloaded(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_background_reads_share_cached_bytes_with_fetcher_clones() {
        let path = std::env::temp_dir().join(format!(
            "proteus-prefetch-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, [4, 5, 6]).unwrap();
        let url = Url::from_file_path(&path).unwrap().to_string();
        tokio::task::LocalSet::new()
            .run_until(async {
                let fetcher = ReqwestFetcher::new();
                let demand = fetcher.clone();
                fetcher.finish_plan(fetcher.begin_plan(), vec![url.clone()]);
                tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    while demand.preloaded(&url).is_none() {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                std::fs::remove_file(&path).unwrap();
                assert_eq!(demand.fetch_bytes(&url).await.unwrap(), [4, 5, 6]);
                fetcher.finish_plan(fetcher.begin_plan(), Vec::new());
                assert!(demand.preloaded(&url).is_none());
            })
            .await;
        if path.exists() {
            std::fs::remove_file(path).unwrap();
        }
    }
}
