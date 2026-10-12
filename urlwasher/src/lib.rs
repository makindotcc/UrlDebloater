pub mod clearurls;
pub mod redirect_resolver;
pub mod text_washer;

use crate::{
    clearurls::{ClearUrlResult, ClearUrls},
    redirect_resolver::{DefaultRedirectResolver, RedirectResolver},
};
use anyhow::Context;
use log::debug;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::{num::NonZeroUsize, sync::Mutex};
use url::Url;

pub const PUBLIC_MIXER_INSTANCE: &str = "https://urldebloater.makin.cc/";

const ALLOWED_REDIRECT_HOSTS: &[&str] = &["vm.tiktok.com", "on.soundcloud.com"];

pub struct UrlWasher<R = DefaultRedirectResolver>
where
    R: RedirectResolver,
{
    redirect_resolver: R,
    clear_urls: ClearUrls,
    cache: Mutex<LruCache<Url, Option<Url>>>,
    config: WasherConfig,
}

impl<R> UrlWasher<R>
where
    R: RedirectResolver,
{
    pub fn new(redirect_resolver: impl Into<R>) -> Self {
        Self::new_with_config(redirect_resolver, WasherConfig::default())
    }

    pub fn new_with_config(redirect_resolver: impl Into<R>, config: WasherConfig) -> Self {
        Self {
            clear_urls: ClearUrls::new(
                serde_json::from_str(include_str!("test_rules.json")).unwrap(),
            ),
            redirect_resolver: redirect_resolver.into(),
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap())),
            config,
        }
    }

    pub async fn wash(&self, url: &Url, options: WashOptions) -> anyhow::Result<Option<Url>> {
        if let Some(cached) = self.cache.lock().unwrap().get(url) {
            debug!("Serving washed url {} from cache.", url.to_string());
            return Ok(cached.to_owned());
        }
        let clear_url = if options.resolve_redirects && self.resolves_redirects_for(url) {
            debug!("Washing url {} with redirect resolution.", url.to_string());
            self.redirect_resolver.resolve(url.clone()).await?
        } else {
            url.clone()
        };
        let clear_url = match self
            .clear_urls
            .clear(&clear_url)
            .context("parse sanitized url")?
        {
            ClearUrlResult::Blocked => None,
            ClearUrlResult::Cleaned(c) | ClearUrlResult::Redirected(c) => Some(c),
            ClearUrlResult::Unchanged => Some(clear_url),
        };
        let clear_url = clear_url.map(|url| self.replace_host(&url)).transpose()?;

        self.cache
            .lock()
            .unwrap()
            .put(url.to_owned(), clear_url.clone());
        Ok(clear_url)
    }

    pub fn resolves_redirects_for(&self, url: &Url) -> bool {
        if url.scheme() != "http" && url.scheme() != "https" {
            return false;
        }
        url.domain()
            .is_some_and(|domain| ALLOWED_REDIRECT_HOSTS.contains(&domain))
    }

    /// Replaces the host of the given URL based on the configured policy.
    /// Used to improve the appearance of embeds in link previews in apps like discord.
    fn replace_host(&self, url: &Url) -> anyhow::Result<Url> {
        let mut new_url = url.clone();
        let Some(domain) = url.domain() else {
            return Ok(new_url);
        };
        let new_host = match domain {
            "twitter.com" | "x.com" => match self.config.twitter_policy {
                TwitterPolicy::Original => None,
                TwitterPolicy::VxTwitter => Some("vxtwitter.com"),
                TwitterPolicy::FxTwitter => Some("fxtwitter.com"),
            },
            _ => None,
        };
        if let Some(new_host) = new_host {
            new_url.set_host(Some(new_host))?;
        }
        Ok(new_url)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasherConfig {
    twitter_policy: TwitterPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TwitterPolicy {
    Original,
    VxTwitter,
    FxTwitter,
}

impl Default for WasherConfig {
    fn default() -> Self {
        Self {
            twitter_policy: TwitterPolicy::FxTwitter,
        }
    }
}

pub struct WashOptions {
    pub resolve_redirects: bool,
}

impl Default for WashOptions {
    fn default() -> Self {
        Self {
            resolve_redirects: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{RedirectResolver, UrlWasher, WashOptions};
    use anyhow::Context;
    use std::{collections::HashMap, future::Future};
    use url::Url;

    #[derive(Default)]
    struct FakeRedirectResolver {
        redirects: HashMap<Url, Url>,
    }

    impl RedirectResolver for FakeRedirectResolver {
        fn resolve(&self, url: Url) -> impl Future<Output = anyhow::Result<Url>> + Send {
            async move {
                self.redirects
                    .get(&url)
                    .cloned()
                    .with_context(|| format!("missing fake redirect for {url}"))
            }
        }
    }

    #[tokio::test]
    async fn test_cleaning() {
        let washer: UrlWasher<FakeRedirectResolver> = UrlWasher::new(FakeRedirectResolver {
            redirects: HashMap::from_iter([
                (
                    Url::parse("https://vm.tiktok.com/ZGJoJs8jb/").unwrap(),
                    Url::parse("https://www.tiktok.com/@/video/7297742182851611936?utm_source=x")
                        .unwrap(),
                ),
                (
                    Url::parse("https://on.soundcloud.com/VLwCL").unwrap(),
                    Url::parse("https://soundcloud.com/djwipeoutnxc/i-c-right-thru-2-u").unwrap(),
                ),
            ]),
            ..Default::default()
        });
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
                "https://vm.tiktok.com/ZGJoJs8jb/",
                "https://www.tiktok.com/@/video/7297742182851611936",
            ),
            (
                "https://on.soundcloud.com/VLwCL",
                "https://soundcloud.com/djwipeoutnxc/i-c-right-thru-2-u",
            ),
        ];

        for (dirty, clean) in tests {
            let dirty_url = Url::parse(&dirty).expect(dirty);
            let clean_url = Url::parse(&clean).expect(clean);
            assert_eq!(
                clean_url.to_string(),
                washer
                    .wash(&dirty_url, WashOptions::default())
                    .await
                    .expect(dirty)
                    .expect(dirty)
                    .to_string(),
                "Invalid wash result of dirty url {dirty}"
            );
        }
    }
}
