pub mod clearurls;
#[cfg(feature = "uniffi")]
pub mod ffi;
pub mod redirect_resolver;
pub mod text_washer;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

use crate::{
    clearurls::{ClearUrlResult, ClearUrls},
    redirect_resolver::{DefaultRedirectResolver, RedirectResolver},
};
use anyhow::Context;
use log::debug;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::{num::NonZeroUsize, sync::Mutex};
use strum::{EnumDiscriminants, EnumIter};
use url::Url;

pub const PUBLIC_MIXER_INSTANCE: &str = "https://urldebloater.makin.cc/";

const ALLOWED_REDIRECT_HOSTS: &[&str] = &["vm.tiktok.com", "on.soundcloud.com"];

pub struct UrlWasher<R = DefaultRedirectResolver>
where
    R: RedirectResolver,
{
    redirect_resolver: R,
    cache: Mutex<LruCache<Url, Option<Url>>>,
    config: WasherConfig,
}

impl<R> UrlWasher<R>
where
    R: RedirectResolver,
{
    pub fn new(redirect_resolver: impl Into<R>, config: WasherConfig) -> Self {
        Self {
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
        let clear_url = match &self.config.clear_urls {
            Some(clear_urls) => {
                match clear_urls
                    .clear(&clear_url)
                    .context("parse sanitized url")?
                {
                    ClearUrlResult::Blocked => None,
                    ClearUrlResult::Cleaned(c) | ClearUrlResult::Redirected(c) => Some(c),
                    ClearUrlResult::Unchanged => Some(clear_url),
                }
            }
            None => Some(clear_url),
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
            "twitter.com" | "x.com" if is_twitter_post_path(url.path()) => {
                match self.config.twitter_policy {
                    TwitterPolicy::Original => None,
                    TwitterPolicy::VxTwitter => Some("vxtwitter.com"),
                    TwitterPolicy::FxTwitter => Some("fxtwitter.com"),
                }
            }
            _ => None,
        };
        if let Some(new_host) = new_host {
            new_url.set_host(Some(new_host))?;
        }
        Ok(new_url)
    }

    pub fn clear_urls(&self) -> Option<&ClearUrls> {
        self.config.clear_urls.as_ref()
    }
}

// https://fxtwitter.com/Foo/status/12345
fn is_twitter_post_path(path: &str) -> bool {
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    segments.len() >= 3 && segments[1] == "status"
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter), cfg_attr(feature = "uniffi", derive(uniffi::Enum)))]
pub enum RedirectWashPolicy {
    /// Do not resolve redirection.
    Ignore,
    /// Resolve redirection locally.
    ///
    /// Exposes your IP address that can be corellated with you.
    #[default]
    Locally,
    /// Resolve redirection using urldebloater-mixer.
    ///
    /// Exposes link to person who is running mixer instance you set
    /// (not so scary for tiktoks tho).
    ViaMixer { mixer_url: Url },
}

impl RedirectWashPolicy {
    pub fn mixer_url(&self) -> Option<&Url> {
        match self {
            RedirectWashPolicy::ViaMixer { mixer_url } => Some(mixer_url),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct WasherConfig {
    pub clear_urls: Option<ClearUrls>,
    pub twitter_policy: TwitterPolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, EnumIter)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum TwitterPolicy {
    #[default]
    Original,
    VxTwitter,
    FxTwitter,
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
pub(crate) mod tests {
    use crate::{
        clearurls::ClearUrls, is_twitter_post_path, RedirectResolver, TwitterPolicy, UrlWasher,
        WashOptions, WasherConfig,
    };
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

    pub(crate) fn test_urlwasher_config() -> WasherConfig {
        WasherConfig {
            clear_urls: Some(ClearUrls::new(
                serde_json::from_str(include_str!("../../../assets/cleanurls_rules.json")).unwrap(),
            )),
            twitter_policy: TwitterPolicy::default(),
        }
    }

    #[tokio::test]
    async fn test_cleaning() {
        let redirect_resolver = FakeRedirectResolver {
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
        };
        let washer: UrlWasher<FakeRedirectResolver> =
            UrlWasher::new(redirect_resolver, test_urlwasher_config());
        let tests = [
            (
                "https://youtu.be/lSwnPoo9ZK0?si=TrackingParamValue&t=65",
                "https://youtu.be/lSwnPoo9ZK0?t=65",
            ),
            (
                "https://music.youtube.com/watch?v=lSwnPoo9ZK0&SI=ETK0gAaXYGNy2aJ6",
                "https://music.youtube.com/watch?v=lSwnPoo9ZK0",
            ),
            (
                "https://www.youtube.com/watch?v=I2W4FepDhyw&list=PLrHjf8BKFvfq1kO8bhqJxfACy0O3kyMu7&index=2",
                "https://www.youtube.com/watch?v=I2W4FepDhyw&list=PLrHjf8BKFvfq1kO8bhqJxfACy0O3kyMu7&index=2",
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

    #[test]
    fn test_is_twitter_post_path() {
        assert!(is_twitter_post_path("/foo/status/12345823"));
        assert!(is_twitter_post_path("/foo/status/12345"));
        assert!(!is_twitter_post_path("/foo/other/12345"));
        assert!(!is_twitter_post_path("/foo/status"));
    }
}
