use std::{path::PathBuf, sync::Arc, time::Duration};

use log::error;
use thiserror::Error;
use url::Url;

use crate::{
    clearurls::{self, ClearUrls, ReqwestCatalogLoader},
    redirect_resolver::{
        DefaultRedirectResolver, WithMixerRedirectResolver, WithoutMixerRedirectResolver,
    },
    text_washer::TextWasher,
    RedirectWashPolicyDiscriminants, TwitterPolicy, UrlWasher, WasherConfig,
};

const CLEARURLS_CACHE_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Error, uniffi::Error)]
pub enum UrlWasherInitError {
    #[error("mixer URL is missing")]
    MissingMixerUrl,
    #[error("mixer URL is invalid")]
    InvalidMixerUrl,
}

#[derive(uniffi::Object)]
pub struct MobileUrlWasher {
    inner: TextWasher,
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn create_url_washer(
    redirect_policy: RedirectWashPolicyDiscriminants,
    mixer_url: Option<String>,
    use_clear_urls: bool,
    clearurls_cache_path: String,
    twitter_policy: TwitterPolicy,
) -> Result<Arc<MobileUrlWasher>, UrlWasherInitError> {
    let redirect_resolver = match redirect_policy {
        RedirectWashPolicyDiscriminants::Ignore => DefaultRedirectResolver::None,
        RedirectWashPolicyDiscriminants::Locally => WithoutMixerRedirectResolver::default().into(),
        RedirectWashPolicyDiscriminants::ViaMixer => {
            let mixer_url = mixer_url
                .as_deref()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .ok_or(UrlWasherInitError::MissingMixerUrl)?;
            let mixer_url =
                Url::parse(mixer_url).map_err(|_| UrlWasherInitError::InvalidMixerUrl)?;
            WithMixerRedirectResolver::new(mixer_url).into()
        }
    };
    let clear_urls = if use_clear_urls {
        let catalog = clearurls::cache::load(
            &ReqwestCatalogLoader,
            PathBuf::from(clearurls_cache_path),
            CLEARURLS_CACHE_DURATION,
        )
        .await
        .unwrap_or_else(|(catalog, err)| {
            error!("Failed to load clearurls catalog from cache: {err:?}");
            catalog
        });
        Some(ClearUrls::new(catalog))
    } else {
        None
    };

    Ok(Arc::new(MobileUrlWasher {
        inner: TextWasher {
            url_washer: UrlWasher::new(
                redirect_resolver,
                WasherConfig {
                    clear_urls,
                    twitter_policy,
                },
            ),
        },
    }))
}

#[uniffi::export(async_runtime = "tokio")]
impl MobileUrlWasher {
    pub async fn wash(&self, text: String) -> String {
        self.inner.wash(&text).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_path(test_name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("urlwasher-ffi-{test_name}-{}", std::process::id()))
            .join("clearurls-cache.json")
    }

    #[tokio::test]
    async fn washes_text_using_cached_clearurls_without_network() {
        let cache_path = cache_path("cached-catalog");
        let cache_dir = cache_path.parent().unwrap();
        let _ = std::fs::remove_dir_all(cache_dir);
        std::fs::create_dir_all(cache_dir).unwrap();
        std::fs::write(
            &cache_path,
            include_str!("../../../assets/cleanurls_rules.json"),
        )
        .unwrap();
        let washer = create_url_washer(
            RedirectWashPolicyDiscriminants::Ignore,
            None,
            true,
            cache_path.to_string_lossy().into_owned(),
            TwitterPolicy::Original,
        )
        .await
        .unwrap();

        let washed = washer
            .wash("https://music.youtube.com/watch?v=lSwnPoo9ZK0&si=ETK0gAaXYGNy2aJ6".to_string())
            .await;

        assert_eq!("https://music.youtube.com/watch?v=lSwnPoo9ZK0", washed);
        std::fs::remove_dir_all(cache_dir).unwrap();
    }

    #[tokio::test]
    async fn applies_twitter_policy() {
        let washer = create_url_washer(
            RedirectWashPolicyDiscriminants::Ignore,
            None,
            false,
            String::new(),
            TwitterPolicy::VxTwitter,
        )
        .await
        .unwrap();

        let washed = washer
            .wash("https://x.com/user/status/123?s=46".to_string())
            .await;

        assert_eq!("https://vxtwitter.com/user/status/123?s=46", washed);
    }

    #[tokio::test]
    async fn returns_structured_mixer_configuration_errors() {
        let missing = create_url_washer(
            RedirectWashPolicyDiscriminants::ViaMixer,
            None,
            false,
            String::new(),
            TwitterPolicy::Original,
        )
        .await;
        assert!(matches!(missing, Err(UrlWasherInitError::MissingMixerUrl)));

        let invalid = create_url_washer(
            RedirectWashPolicyDiscriminants::ViaMixer,
            Some("not a URL".to_string()),
            false,
            String::new(),
            TwitterPolicy::Original,
        )
        .await;
        assert!(matches!(invalid, Err(UrlWasherInitError::InvalidMixerUrl)));
    }
}
