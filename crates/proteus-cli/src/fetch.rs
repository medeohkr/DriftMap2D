use proteus_core::{BoxFuture, FetchError, TileFetcher};
use url::Url;

pub struct ReqwestFetcher {
    client: reqwest::Client,
}

impl ReqwestFetcher {
    pub fn new() -> Self {
        Self { client: reqwest::Client::new() }
    }
}

impl TileFetcher for ReqwestFetcher {
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
                    FetchError::Network(format!(
                        "file read failed: {}: {e}",
                        path.display()
                    ))
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