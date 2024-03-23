use anyhow::{anyhow, Context};
use lru::LruCache;
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, num::NonZeroUsize, sync::OnceLock};
use tokio::sync::Mutex;
use tracing::debug;
use url::Url;

pub mod text_washer;

pub const PUBLIC_MIXER_INSTANCE: &str = "https://urldebloater.makin.cc/";

static DEFAULT_RULE_SET: OnceLock<Vec<DirtyUrlRule>> = OnceLock::new();

pub type RuleName = String;

pub fn rule_set() -> &'static Vec<DirtyUrlRule> {
    DEFAULT_RULE_SET.get_or_init(|| {
        vec![
            DirtyUrlRule {
                name: "youtu.be".to_string(),
                domains: vec!["youtu.be".to_string()],
                washing_programs: vec![WashingProgram::remove_some_params(&["si"])],
                ..Default::default()
            },
            DirtyUrlRule {
                name: "youtube.com & music.youtube.com".to_string(),
                domains: vec![
                    "youtube.com".to_string(),
                    "www.youtube.com".to_string(),
                    "music.youtube.com".to_string(),
                ],
                washing_programs: vec![WashingProgram::remove_some_params(&["si"])],
                ..Default::default()
            },
            #[warn(clippy::needless_update)]
            DirtyUrlRule {
                name: "twitter.com".to_string(),
                domains: vec!["twitter.com".to_string(), "x.com".to_string()],
                path_pattern: vec![],
                washing_programs: vec![WashingProgram::RemoveAllParams],
                ..Default::default()
            },
            DirtyUrlRule {
                name: "vm.tiktok.com".to_string(),
                domains: vec!["vm.tiktok.com".to_string()],
                washing_programs: vec![
                    WashingProgram::ResolveRedirection,
                    WashingProgram::RemoveAllParams,
                ],
                ..Default::default()
            },
            DirtyUrlRule {
                name: "on.soundcloud.com".to_string(),
                domains: vec!["on.soundcloud.com".to_string()],
                washing_programs: vec![
                    WashingProgram::ResolveRedirection,
                    WashingProgram::RemoveAllParams,
                ],
                ..Default::default()
            },
        ]
    })
}

pub struct UrlWasher {
    cache: Mutex<LruCache<Url, Url>>,
    http_client: reqwest::Client,
    config: UrlWasherConfig,
}

impl Default for UrlWasher {
    fn default() -> Self {
        Self::new(UrlWasherConfig::default())
    }
}

impl UrlWasher {
    pub fn new(config: UrlWasherConfig) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap())),
            http_client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .redirect(Policy::none())
                .build()
                .unwrap(),
            config,
        }
    }

    pub async fn wash(&self, url: &Url) -> anyhow::Result<Option<Url>> {
        if url.scheme() != "http" && url.scheme() != "https" {
            return Ok(None);
        }
        if let Some(cached) = self.cache.lock().await.get(url) {
            debug!("Serving washed url {} from cache.", url.to_string());
            return Ok(Some(cached.to_owned()));
        }
        let domain = match url.domain() {
            Some(domain) => domain,
            None => return Ok(None),
        };
        let rules = rule_set();
        let matching_rule = match rules
            .iter()
            .find(|rule| rule.matches_domain(domain) && rule.matches_path(url))
        {
            Some(r) => r,
            None => return Ok(None),
        };
        let mut laundry = url.to_owned();
        for washing_program in matching_rule.washing_programs.iter() {
            laundry = match washing_program {
                WashingProgram::ResolveRedirection => {
                    let policy = self
                        .config
                        .redirect_policy
                        .get(&matching_rule.name)
                        .unwrap_or(&RedirectWashPolicy::Ignore);
                    match resolve_redirect(
                        &self.http_client,
                        laundry,
                        policy,
                        &self.config.mixer_instance,
                    )
                    .await
                    {
                        Ok(Ok(url)) | Ok(Err(url)) => url,
                        Err(err) => return Err(err),
                    }
                }
                WashingProgram::RemoveSomeParams(params) => remove_query_params(&laundry, params),
                WashingProgram::RemoveAllParams => {
                    laundry.set_query(None);
                    laundry
                }
            };
        }
        self.cache.lock().await.put(url.to_owned(), laundry.clone());
        Ok(Some(laundry))
    }
}

fn remove_query_params(url: &Url, params: &[String]) -> Url {
    let mut debloated_url = url.clone();
    debloated_url.query_pairs_mut().clear();
    let debloated_query = url
        .query_pairs()
        .filter(|(query_key, _)| params.iter().all(|param| param != query_key));
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

async fn resolve_redirect(
    http_client: &reqwest::Client,
    url: Url,
    policy: &RedirectWashPolicy,
    mixer_instance: &Option<Url>,
) -> anyhow::Result<Result<Url, Url>> {
    match policy {
        RedirectWashPolicy::Ignore => Ok(Err(url)),
        RedirectWashPolicy::Locally => {
            let resp = http_client.get(url).send().await?;
            let location = resp
                .headers()
                .get("location")
                .context("missing location header")?
                .to_str()
                .context("invalid location header")?;
            Url::parse(location).context("parse location url").map(Ok)
        }
        RedirectWashPolicy::ViaMixer => {
            let mixer_instance = mixer_instance
                .as_ref()
                .context("undefined mixer instance")?;
            let mut wash_url = mixer_instance.clone();
            wash_url.set_path("wash");
            let resp = http_client
                .get(wash_url)
                .query(&[("url", url.to_string())])
                .send()
                .await
                .context("send mixer requewst")?;
            if !resp.status().is_success() {
                return Err(anyhow!("Invalid mixer response status: {}", resp.status()));
            }
            Url::parse(&resp.text().await.context("read mixer response url")?)
                .context("parse mixer response url")
                .map(Ok)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UrlWasherConfig {
    pub mixer_instance: Option<Url>,
    pub redirect_policy: HashMap<RuleName, RedirectWashPolicy>,
}

impl Default for UrlWasherConfig {
    fn default() -> Self {
        Self {
            mixer_instance: Default::default(),
            redirect_policy: HashMap::from_iter(
                rule_set()
                    .iter()
                    .filter(|rule| {
                        rule.washing_programs
                            .contains(&WashingProgram::ResolveRedirection)
                    })
                    .flat_map(|rule| {
                        rule.domains
                            .iter()
                            .map(|domain| (domain.to_owned(), RedirectWashPolicy::Locally))
                    }),
            ),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum RedirectWashPolicy {
    /// Do not resolve redirection.
    Ignore,
    /// Resolve redirection locally.
    ///
    /// Exposes your IP address that can be corellated with you.
    Locally,
    /// Resolve redirection using urldebloater-mixer.
    ///
    /// Exposes link to person who is running mixer instance you set
    /// (not so scary for tiktoks tho).
    ViaMixer,
}

impl Display for RedirectWashPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            RedirectWashPolicy::Ignore => "ignore",
            RedirectWashPolicy::Locally => "locally",
            RedirectWashPolicy::ViaMixer => "via mixer",
        })
    }
}

#[derive(Default)]
#[non_exhaustive]
pub struct DirtyUrlRule {
    pub name: String,
    pub domains: Vec<String>,
    pub path_pattern: Vec<Option<String>>,
    pub washing_programs: Vec<WashingProgram>,
}

impl DirtyUrlRule {
    pub fn matches_domain(&self, domain: &str) -> bool {
        self.domains
            .iter()
            .any(|dirty_domain| dirty_domain == domain)
    }

    pub fn matches_path(&self, url: &Url) -> bool {
        if self.path_pattern.is_empty() {
            return true;
        }
        let segments = match url.path_segments() {
            Some(segments) => segments,
            None => return false,
        };
        segments
            .zip(&self.path_pattern)
            .all(|(actual, template)| match template {
                Some(template) => actual == template,
                None => true,
            })
    }
}

#[derive(PartialEq, Eq)]
pub enum WashingProgram {
    ResolveRedirection,
    RemoveSomeParams(Vec<String>),
    RemoveAllParams,
}

impl WashingProgram {
    pub fn remove_some_params(values: &[&str]) -> Self {
        Self::RemoveSomeParams(values.iter().map(|s| String::from(*s)).collect())
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use crate::{UrlWasher, UrlWasherConfig};

    #[tokio::test]
    async fn test_cleaning() {
        let washer = UrlWasher::new(UrlWasherConfig::default());
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
                "https://www.tiktok.com/@i0ki.clips/video/7297742182851611936",
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
