use anyhow::{anyhow, Context};
use reqwest::header;
use std::{future::Future, time::Duration};
use url::Url;

pub trait RedirectResolver: Send + Sync {
    fn resolve(&self, url: Url) -> impl Future<Output = anyhow::Result<Url>> + Send;
}

pub enum DefaultRedirectResolver {
    None,
    WithMixer(WithMixerRedirectResolver),
    WithoutMixer(WithoutMixerRedirectResolver),
}

impl RedirectResolver for DefaultRedirectResolver {
    fn resolve(&self, url: Url) -> impl Future<Output = anyhow::Result<Url>> + Send {
        async move {
            match self {
                Self::None => Ok(url),
                Self::WithMixer(resolver) => resolver.resolve(url).await,
                Self::WithoutMixer(resolver) => resolver.resolve(url).await,
            }
        }
    }
}

pub struct WithoutMixerRedirectResolver {
    client: reqwest::Client,
}

impl Default for WithoutMixerRedirectResolver {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .user_agent(
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                    (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36",
                )
                .pool_idle_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(5))
                .build()
                .expect("build HTTP client"),
        }
    }
}

impl RedirectResolver for WithoutMixerRedirectResolver {
    fn resolve(&self, url: Url) -> impl Future<Output = anyhow::Result<Url>> + Send {
        async move {
            let resp = self
                .client
                .get(url)
                .send()
                .await
                .context("send local redirect request")?;
            let location = resp
                .headers()
                .get(header::LOCATION)
                .context("missing location header")?
                .to_str()
                .context("invalid location header")?;
            Url::parse(location).context("parse location url")
        }
    }
}

impl From<WithoutMixerRedirectResolver> for DefaultRedirectResolver {
    fn from(resolver: WithoutMixerRedirectResolver) -> Self {
        Self::WithoutMixer(resolver)
    }
}

pub struct WithMixerRedirectResolver {
    client: reqwest::Client,
    mixer_instance: Url,
}

impl WithMixerRedirectResolver {
    pub fn new(mixer_instance: Url) -> Self {
        Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("urlwasher")
                .timeout(Duration::from_secs(5))
                .build()
                .expect("build HTTP client"),
            mixer_instance,
        }
    }
}

impl RedirectResolver for WithMixerRedirectResolver {
    fn resolve(&self, url: Url) -> impl Future<Output = anyhow::Result<Url>> + Send {
        async move {
            let mut wash_url = self.mixer_instance.clone();
            wash_url.set_path("wash");
            wash_url.query_pairs_mut().append_pair("url", url.as_str());
            let resp = self
                .client
                .get(wash_url)
                .send()
                .await
                .context("send mixer request")?;
            if !resp.status().is_success() {
                return Err(anyhow!("Invalid mixer response status: {}", resp.status()));
            }
            Url::parse(&resp.text().await.context("read mixer response url")?)
                .context("parse mixer response url")
        }
    }
}

impl From<WithMixerRedirectResolver> for DefaultRedirectResolver {
    fn from(resolver: WithMixerRedirectResolver) -> Self {
        Self::WithMixer(resolver)
    }
}
