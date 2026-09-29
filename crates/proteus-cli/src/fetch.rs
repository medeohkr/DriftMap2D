use proteus_core::{BoxFuture, FetchError, TileFetcher};

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