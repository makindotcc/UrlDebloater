use crate::{
    redirect_resolver::{DefaultRedirectResolver, RedirectResolver},
    UrlWasher, WashOptions,
};
use log::{debug, error};
use url::Url;

const MAX_REDIRECTING_URLS: usize = 10;

/// A utility to debloat urls in arbitrary text.
pub struct TextWasher<R = DefaultRedirectResolver>
where
    R: RedirectResolver,
{
    pub url_washer: UrlWasher<R>,
}

impl<R> TextWasher<R>
where
    R: RedirectResolver,
{
    pub async fn wash(&self, text: &str) -> String {
        let mut original_separators = Vec::new();
        let parts = text
            .split(|c: char| {
                let is_whitespace = c.is_whitespace();
                if is_whitespace {
                    original_separators.push(c);
                }
                is_whitespace
            })
            .collect::<Vec<_>>();

        let mut washed_parts = Vec::with_capacity(parts.len());
        let mut redirecting_urls = 0;
        for part in parts {
            if !part.starts_with("http://") && !part.starts_with("https://") {
                washed_parts.push(part.to_string());
                continue;
            }
            let url = match Url::parse(part) {
                Ok(url) => url,
                Err(_) => {
                    washed_parts.push(part.to_string());
                    continue;
                }
            };
            debug!("Washing part of text: {url}");
            let resolve_redirects = if self.url_washer.resolves_redirects_for(&url) {
                redirecting_urls += 1;
                redirecting_urls <= MAX_REDIRECTING_URLS
            } else {
                true
            };
            let washed = match self
                .url_washer
                .wash(&url, WashOptions { resolve_redirects })
                .await
            {
                Ok(Some(clean_url)) => clean_url.to_string(),
                Ok(None) => part.to_string(),
                Err(err) => {
                    error!("Could not wash url '{}': {:?}", part, err);
                    part.to_string()
                }
            };
            washed_parts.push(washed);
        }

        let mut patched = String::new();
        for (index, part) in washed_parts.into_iter().enumerate() {
            patched.push_str(&part);
            if let Some(separator) = original_separators.get(index) {
                patched.push(*separator);
            }
        }
        patched
    }
}

#[cfg(test)]
mod tests {
    use crate::{tests::test_urlwasher_config, RedirectResolver, UrlWasher};
    use anyhow::Context;
    use std::{
        collections::HashMap,
        future::Future,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    use url::Url;

    use super::TextWasher;

    #[derive(Clone, Default)]
    struct FakeRedirectResolver {
        redirects: HashMap<Url, Url>,
        calls: Arc<AtomicUsize>,
    }

    impl RedirectResolver for FakeRedirectResolver {
        fn resolve(&self, url: Url) -> impl Future<Output = anyhow::Result<Url>> + Send {
            async move {
                self.calls.fetch_add(1, Ordering::Relaxed);
                self.redirects
                    .get(&url)
                    .cloned()
                    .with_context(|| format!("missing fake redirect for {url}"))
            }
        }
    }

    #[tokio::test]
    pub async fn removes_tracking_without_counting_against_redirect_limit() {
        let text_washer: TextWasher<FakeRedirectResolver> = TextWasher {
            url_washer: UrlWasher::new(FakeRedirectResolver::default(), test_urlwasher_config()),
        };

        let dirty = (0..12)
            .map(|index| format!("https://music.youtube.com/watch?v={index}&si=TRACKING{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let clean = (0..12)
            .map(|index| format!("https://music.youtube.com/watch?v={index}"))
            .collect::<Vec<_>>()
            .join(" ");

        assert_eq!(clean, text_washer.wash(&dirty).await);
    }

    #[tokio::test]
    pub async fn limits_redirect_resolution_to_first_ten_redirecting_urls() {
        let calls = Arc::new(AtomicUsize::new(0));
        let redirects = (0..12)
            .map(|index| {
                (
                    Url::parse(&format!("https://vm.tiktok.com/{index}/")).unwrap(),
                    Url::parse(&format!(
                        "https://www.tiktok.com/@test/video/{index}?utm_source=x"
                    ))
                    .unwrap(),
                )
            })
            .collect();
        let text_washer: TextWasher<FakeRedirectResolver> = TextWasher {
            url_washer: UrlWasher::new(
                FakeRedirectResolver {
                    redirects,
                    calls: Arc::clone(&calls),
                },
                test_urlwasher_config(),
            ),
        };

        let dirty = (0..12)
            .map(|index| format!("https://vm.tiktok.com/{index}/"))
            .collect::<Vec<_>>()
            .join(" ");
        let clean = (0..10)
            .map(|index| format!("https://www.tiktok.com/@test/video/{index}"))
            .chain((10..12).map(|index| format!("https://vm.tiktok.com/{index}/")))
            .collect::<Vec<_>>()
            .join(" ");

        assert_eq!(clean, text_washer.wash(&dirty).await);
        assert_eq!(10, calls.load(Ordering::Relaxed));
    }
}
