use anyhow::{Context, anyhow, bail};
use futures_channel::oneshot;
use std::future::Future;
use url::Url;
use urldebloater_urlwasher::{
    TwitterPolicy, UrlWasher, WasherConfig,
    clearurls::{ClearUrls, DataCatalog},
    redirect_resolver::{DefaultRedirectResolver, RedirectResolver},
    text_washer::TextWasher,
};
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};

const CLEARURLS_RULES: &str = include_str!(concat!(
    env!("CARGO_WORKSPACE_DIR"),
    "assets/cleanurls_rules.json"
));

#[wasm_bindgen]
pub struct UrlCleaner {
    with_short_links: TextWasher<MixerRedirectResolver>,
    without_short_links: TextWasher<DefaultRedirectResolver>,
}

#[wasm_bindgen]
impl UrlCleaner {
    #[wasm_bindgen(constructor)]
    pub fn new(mixer_url: &str) -> Result<UrlCleaner, JsValue> {
        let catalog = serde_json::from_str::<DataCatalog>(CLEARURLS_RULES)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let mixer_url =
            Url::parse(mixer_url).map_err(|error| JsValue::from_str(&error.to_string()))?;
        if !matches!(mixer_url.scheme(), "http" | "https") {
            return Err(JsValue::from_str("mixer URL must use http or https"));
        }

        Ok(Self {
            with_short_links: TextWasher {
                url_washer: UrlWasher::new(
                    MixerRedirectResolver::new(mixer_url),
                    WasherConfig {
                        clear_urls: Some(ClearUrls::new(catalog.clone())),
                        twitter_policy: TwitterPolicy::Original,
                    },
                ),
            },
            without_short_links: TextWasher {
                url_washer: UrlWasher::new(
                    DefaultRedirectResolver::None,
                    WasherConfig {
                        clear_urls: Some(ClearUrls::new(catalog)),
                        twitter_policy: TwitterPolicy::Original,
                    },
                ),
            },
        })
    }

    pub async fn wash(&self, text: &str) -> String {
        self.with_short_links.wash(text).await
    }

    pub async fn wash_without_short_links(&self, text: &str) -> String {
        self.without_short_links.wash(text).await
    }
}

struct MixerRedirectResolver {
    mixer_url: Url,
}

impl MixerRedirectResolver {
    fn new(mixer_url: Url) -> Self {
        Self { mixer_url }
    }
}

impl RedirectResolver for MixerRedirectResolver {
    fn resolve(&self, url: Url) -> impl Future<Output = anyhow::Result<Url>> + Send {
        let mixer_url = self.mixer_url.clone();
        let (sender, receiver) = oneshot::channel();
        spawn_local(async move {
            let _ = sender.send(fetch_mixer(mixer_url, url).await);
        });

        async move {
            receiver
                .await
                .map_err(|_| anyhow!("mixer request was cancelled"))?
        }
    }
}

async fn fetch_mixer(mut mixer_url: Url, url: Url) -> anyhow::Result<Url> {
    mixer_url.set_path("wash");
    mixer_url.query_pairs_mut().append_pair("url", url.as_str());

    let window = web_sys::window().context("browser window is unavailable")?;
    let response = JsFuture::from(window.fetch_with_str(mixer_url.as_str()))
        .await
        .map_err(|error| anyhow!("mixer fetch failed: {error:?}"))?;
    let response = response
        .dyn_into::<web_sys::Response>()
        .map_err(|_| anyhow!("mixer fetch returned an unexpected value"))?;
    if !response.ok() {
        bail!("mixer returned HTTP {}", response.status());
    }

    let body = JsFuture::from(
        response
            .text()
            .map_err(|error| anyhow!("read mixer response: {error:?}"))?,
    )
    .await
    .map_err(|error| anyhow!("read mixer response: {error:?}"))?;
    let body = body
        .as_string()
        .context("mixer response body is not text")?;

    Url::parse(body.trim()).context("parse mixer response URL")
}
