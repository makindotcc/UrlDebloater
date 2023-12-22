use std::{ops::Deref, num::NonZeroUsize};

use anyhow::Context;
use lru::LruCache;
use reqwest::redirect::Policy;
use tokio::sync::Mutex;
use tracing::debug;
use url::Url;

pub mod text_washer;

pub struct UrlWasher {
    cache: Mutex<LruCache<Url, Url>>,
    http_client: reqwest::Client,
}

impl Default for UrlWasher {
    fn default() -> Self {
        Self {
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap())), 
            http_client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .redirect(Policy::none())
                .build()
                .unwrap(),
        }
    }
}

impl UrlWasher {
    pub async fn wash(&self, url: &Url) -> anyhow::Result<Option<Url>> {
        if url.scheme() != "http" && url.scheme() != "https" {
            return Ok(None);
        }
        if let Some(cached) = self.cache.lock().await.get(&url) {
            debug!("Serving washed url {} from cache.", url.to_string());
            return Ok(Some(cached.to_owned()));
        }
        let domain = match url.domain() {
            Some(domain) => domain,
            None => return Ok(None),
        };
        let cleaned = match domain {
            "youtu.be" => Ok(Some(Self::remove_query_params(url, &["si"]))),
            "youtube.com" | "www.youtube.com" | "music.youtube.com" if url.path() == "/watch" => {
                Ok(Some(Self::remove_query_params(url, &["si"])))
            }
            "twitter.com" | "x.com"
                if url
                    .path_segments()
                    .is_some_and(|segments| matches!(segments.skip(1).next(), Some("status"))) =>
            {
                Ok(Some(Self::remove_query_params(url, &["s", "t"])))
            }
            "vm.tiktok.com" => self
                .resolve_redirect(url.to_owned())
                .await
                .map(|mut resolved| {
                    resolved.set_query(None);
                    Some(resolved)
                }),
            _ => return Ok(None),
        };
        if let Ok(Some(cleaned)) = &cleaned {
            self.cache.lock().await.put(url.to_owned(), cleaned.to_owned());
        }
        cleaned
    }

    fn remove_query_params(url: &Url, params: &[&str]) -> Url {
        let mut debloated_url = url.clone();
        debloated_url.query_pairs_mut().clear();
        let debloated_query = url
            .query_pairs()
            .filter(|(query_key, _)| !params.contains(&query_key.deref()));
        for (query_key, query_value) in debloated_query {
            debloated_url
                .query_pairs_mut()
                .append_pair(&query_key, &query_value);
        }
        if let Some("") = debloated_url.query() {
            debloated_url.set_query(None);
        }
        debloated_url
    }

    async fn resolve_redirect(&self, url: Url) -> anyhow::Result<Url> {
        let resp = self.http_client.get(url).send().await?;
        let location = resp
            .headers()
            .get("location")
            .context("missing location header")?
            .to_str()
            .context("invalid location header")?;
        Ok(Url::parse(location).context("parse location url")?)
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use crate::UrlWasher;

    #[tokio::test]
    async fn test_cleaning() {
        let washer = UrlWasher::default();

        let tests = [
            (
                "https://youtu.be/lSwnPoo9ZK0?si=TrackingParamValue&t=65",
                "https://youtu.be/lSwnPoo9ZK0?t=65",
            ),
            (
                "https://music.youtube.com/watch?v=lSwnPoo9ZK0&si=ETK0gAaXYGNy2aJ6",
                "https://music.youtube.com/watch?v=lSwnPoo9ZK0",
            ),
            (
                "https://x.com/sekurak/status/1737942071431073818?s=46&t=eLM_fuufufjf",
                "https://x.com/sekurak/status/1737942071431073818",
            ),
            (
                "https://vm.tiktok.com/ZGJsEDpFN/",
                "https://www.tiktok.com/@python_is_trash/video/7270531341521849605",
            ),
        ];

        for (dirty, clean) in tests {
            let dirty_url = Url::parse(&dirty).expect(dirty);
            let clean_url = Url::parse(&clean).expect(clean);
            assert_eq!(
                clean_url.to_string(),
                washer
                    .wash(&dirty_url)
                    .await
                    .expect(dirty)
                    .expect(dirty)
                    .to_string(),
                "Invalid wash result of dirty url {dirty}"
            );
        }
    }
}
