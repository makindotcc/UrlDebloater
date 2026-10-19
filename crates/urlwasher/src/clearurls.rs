use anyhow::anyhow;
#[cfg(feature = "native")]
use anyhow::ensure;
use anyhow::Context;
#[cfg(feature = "cleanurls-cache")]
use log::error;
use log::warn;
use regex::Regex;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::OnceLock;
use url::{form_urlencoded, Url};

/// Fork of ClearURLs rules, because it looks like the original repository is no longer actively maintained.
const RULES_URL: &str = "https://raw.githubusercontent.com/makindotcc/UrlDebloater/refs/heads/master/assets/cleanurls_rules.json";

#[derive(Debug)]
pub struct ClearUrls {
    catalog: DataCatalog,
}

impl ClearUrls {
    pub fn new(catalog: DataCatalog) -> Self {
        Self { catalog }
    }

    pub fn clear(&self, url: &Url) -> anyhow::Result<ClearUrlResult> {
        let mut current = url.clone();

        for provider in self.catalog.matching_providers(url) {
            let rules = provider
                .rules()
                .with_context(|| format!("compile rules for '{url}'"))?;
            if rules.is_exception(url.as_str()) {
                continue;
            }
            if provider.complete_provider {
                return Ok(ClearUrlResult::Blocked);
            }
            if let Some(redirected) = rules.redirect(url) {
                return Ok(ClearUrlResult::Redirected(redirected));
            }

            rules.clear_query(&mut current);
            rules
                .apply_raw_rules(&mut current)
                .context("apply raw rules")?;
        }

        let result = if &current == url {
            ClearUrlResult::Unchanged
        } else {
            ClearUrlResult::Cleaned(current)
        };
        Ok(result)
    }

    pub fn catalog(&self) -> &DataCatalog {
        &self.catalog
    }
}

pub trait CatalogLoader {
    fn load_catalog(&self) -> impl Future<Output = anyhow::Result<DataCatalog>>;
}

#[cfg(feature = "native")]
pub struct ReqwestCatalogLoader;

#[cfg(feature = "native")]
impl CatalogLoader for ReqwestCatalogLoader {
    fn load_catalog(&self) -> impl Future<Output = anyhow::Result<DataCatalog>> {
        async {
            let response = reqwest::get(RULES_URL)
                .await
                .context("get clearurls data")?;
            ensure!(
                response.status().is_success(),
                "invalid clearurls data status code: {}",
                response.status()
            );
            let catalog = response.text().await.context("get clearurls body")?;
            let catalog = serde_json::from_str::<DataCatalog>(&catalog)
                .context("deserialize clearurls data")?;
            Ok(catalog)
        }
    }
}

#[cfg(feature = "cleanurls-cache")]
pub mod cache {
    use crate::clearurls::{CatalogLoader, DataCatalog};
    use anyhow::Context;
    use log::warn;
    use std::{path::PathBuf, time::Duration};

    pub async fn load(
        loader: &impl CatalogLoader,
        path: PathBuf,
        max_age: Duration,
    ) -> Result<DataCatalog, (DataCatalog, anyhow::Error)> {
        match load_valid_cache(&path, max_age).await {
            Ok(Some(cache)) => return Ok(cache),
            Ok(None) => {}
            Err(err) => {
                warn!("failed to load cache: {:?}", err);
            }
        };
        match loader.load_catalog().await {
            Ok(new_catalog) => {
                if let Err(err) = save_cache(&path, &new_catalog).await {
                    warn!("failed to save new catalog to cache: {:?}", err);
                };
                Ok(new_catalog)
            }
            Err(err) => {
                warn!("failed to load new catalog: {:?}", err);
                match load_cache(&path).await {
                    Ok(cache) => Err((cache, err)),
                    Err(load_err) => {
                        let default_catalog = DataCatalog::hardcoded();
                        Err((
                            default_catalog,
                            load_err.context("load cache after failed catalog load"),
                        ))
                    }
                }
            }
        }
    }

    /// Load the valid cache if it exists and is not outdated.
    /// Returns `Ok(None)` if the cache is missing or outdated.
    async fn load_valid_cache(
        path: &PathBuf,
        max_age: Duration,
    ) -> anyhow::Result<Option<DataCatalog>> {
        let cache_metadata = match tokio::fs::metadata(&path).await {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err).context("get cache metadata"),
        };
        let modified_at = cache_metadata.modified().context("get modified date")?;
        if modified_at.elapsed().unwrap_or_default() > max_age {
            return Ok(None);
        }
        let catalog = load_cache(&path).await.context("load cache")?;
        Ok(Some(catalog))
    }

    async fn load_cache(path: &PathBuf) -> anyhow::Result<DataCatalog> {
        let catalog = tokio::fs::read_to_string(&path)
            .await
            .context("read cache file")?;
        let catalog =
            serde_json::from_str::<DataCatalog>(&catalog).context("deserialize cache data")?;
        Ok(catalog)
    }

    async fn save_cache(path: &PathBuf, catalog: &DataCatalog) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("create cache directory")?;
        }
        let tmp_path = path.with_extension(format!(".tmp{}", std::process::id()));
        tokio::fs::write(
            &tmp_path,
            serde_json::to_string(&catalog).context("serialize catalog")?,
        )
        .await
        .context("write catalog to cache")?;
        tokio::fs::rename(&tmp_path, &path)
            .await
            .context("rename temporary cache file")?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClearUrlResult {
    Unchanged,
    Cleaned(Url),
    Redirected(Url),
    Blocked,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DataCatalog {
    pub providers: BTreeMap<String, Provider>,
}

impl DataCatalog {
    #[cfg(feature = "cleanurls-cache")]
    fn hardcoded() -> Self {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_WORKSPACE_DIR"),
            "assets/cleanurls_rules.json"
        )))
        .unwrap_or_else(|err| {
            error!("Could not load hardcoded cleanurls rules: {err:?}");
            Self {
                providers: BTreeMap::new(),
            }
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    #[serde(deserialize_with = "deserialize_regex")]
    pub url_pattern: CompiledRegex,
    #[serde(default)]
    pub complete_provider: bool,
    #[serde(default)]
    rules: Vec<String>,
    #[serde(default)]
    raw_rules: Vec<String>,
    #[serde(default)]
    referral_marketing: Vec<String>,
    #[serde(default)]
    exceptions: Vec<String>,
    #[serde(default)]
    redirections: Vec<String>,
    #[serde(default)]
    pub force_redirection: bool,
    #[serde(skip)]
    compiled_rules: OnceLock<Result<CompiledRules, String>>,
}

impl DataCatalog {
    pub fn provider(&self, name: &str) -> Option<&Provider> {
        self.providers.get(name)
    }

    fn matching_providers<'a>(&'a self, url: &'a Url) -> impl Iterator<Item = &'a Provider> {
        let url = url.as_str();
        self.providers
            .values()
            .filter(move |provider| provider.url_pattern.compiled.is_match(url))
    }
}

impl Provider {
    fn rules(&self) -> anyhow::Result<&CompiledRules> {
        self.compiled_rules
            .get_or_init(|| compile_rules(self).map_err(|err| err.to_string()))
            .as_ref()
            .map_err(|err| anyhow!("{err}"))
    }
}

#[derive(Debug, Clone)]
struct CompiledRules {
    rules: Vec<Regex>,
    raw_rules: Vec<Regex>,
    referral_marketing: Vec<Regex>,
    exceptions: Vec<Regex>,
    redirections: Vec<Regex>,
}

impl CompiledRules {
    fn is_exception(&self, url: &str) -> bool {
        self.exceptions.iter().any(|rule| rule.is_match(url))
    }

    fn redirect(&self, url: &Url) -> Option<Url> {
        self.redirections
            .iter()
            .filter_map(|rule| rule.captures(url.as_str())?.get(1))
            .find_map(|capture| {
                let query = format!("target={}", capture.as_str());
                let (_, decoded) = form_urlencoded::parse(query.as_bytes()).next()?;
                Url::parse(&decoded)
                    .inspect_err(|err| warn!("Could not parse redirect URL: {err}"))
                    .ok()
            })
    }

    fn clear_query(&self, url: &mut Url) {
        let kept_pairs: Vec<_> = url
            .query_pairs()
            .filter(|(key, _)| !self.should_remove_query_param(key))
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        if kept_pairs.len() == url.query_pairs().count() {
            return;
        }
        if kept_pairs.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(kept_pairs);
        }
    }

    fn should_remove_query_param(&self, key: &str) -> bool {
        self.rules
            .iter()
            .chain(&self.referral_marketing)
            .any(|rule| rule.is_match(key))
    }

    fn apply_raw_rules(&self, url: &mut Url) -> anyhow::Result<()> {
        if self.raw_rules.is_empty() {
            return Ok(());
        }

        let mut cleaned = url.to_string();
        for rule in &self.raw_rules {
            cleaned = rule.replace_all(&cleaned, "").to_string();
        }

        if cleaned != url.as_str() {
            *url = Url::parse(&cleaned)
                .with_context(|| format!("invalid URL after raw rules: {cleaned}"))?;
        }
        Ok(())
    }
}

fn compile_rules(provider: &Provider) -> anyhow::Result<CompiledRules> {
    fn compile_all(patterns: &[String]) -> anyhow::Result<Vec<Regex>> {
        patterns
            .iter()
            .map(|pattern| {
                Regex::new(pattern).with_context(|| format!("invalid pattern '{pattern}'"))
            })
            .collect()
    }

    Ok(CompiledRules {
        rules: compile_all(&provider.rules)?,
        raw_rules: compile_all(&provider.raw_rules)?,
        referral_marketing: compile_all(&provider.referral_marketing)?,
        exceptions: compile_all(&provider.exceptions)?,
        redirections: compile_all(&provider.redirections)?,
    })
}

#[derive(Debug, Serialize, Clone)]
#[serde(transparent)]
pub struct CompiledRegex {
    raw: String,
    #[serde(skip)]
    compiled: Regex,
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<CompiledRegex, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    let compiled = Regex::new(&raw).map_err(de::Error::custom)?;
    Ok(CompiledRegex { raw, compiled })
}

#[cfg(test)]
mod tests {
    use super::{ClearUrlResult, ClearUrls, DataCatalog};
    use url::Url;

    #[test]
    fn deserializes_clearurls_data_catalog() {
        let catalog: DataCatalog =
            serde_json::from_str(include_str!("../../../assets/cleanurls_rules.json")).unwrap();

        let amazon = catalog.provider("amazon").unwrap();
        assert_eq!(
            amazon.url_pattern.compiled.as_str(),
            "^https?:\\/\\/(?:[a-z0-9-]+\\.)*?amazon(?:\\.[a-z]{2,}){1,}"
        );
        assert!(!amazon.complete_provider);
        let amazon_rules = amazon.rules().unwrap();
        assert!(amazon_rules.rules.iter().any(|rule| rule.as_str() == "qid"));
        assert!(amazon_rules
            .raw_rules
            .iter()
            .any(|rule| rule.as_str() == "\\/ref=[^/?]*"));
        assert!(amazon_rules
            .referral_marketing
            .iter()
            .any(|rule| rule.as_str() == "tag"));
        assert!(!amazon.force_redirection);

        let blocked_provider = catalog.provider("fls-na.amazon").unwrap();
        assert!(blocked_provider.complete_provider);
        assert!(blocked_provider.rules().unwrap().rules.is_empty());

        let google = catalog.provider("google").unwrap();
        assert!(google.force_redirection);
    }

    #[test]
    fn compiles_every_catalog_rules() {
        let catalog: DataCatalog =
            serde_json::from_str(include_str!("../../../assets/cleanurls_rules.json")).unwrap();

        for (name, provider) in &catalog.providers {
            provider
                .rules()
                .unwrap_or_else(|err| panic!("provider '{name}' rules: {err:#}"));
        }
    }

    #[test]
    fn clears_query_params() {
        let clear_urls = test_clear_urls();
        let url = Url::parse("https://example.com/page?keep=1&utm_source=feed&ga_test=2").unwrap();

        assert_eq!(
            clear_urls.clear(&url).unwrap(),
            ClearUrlResult::Cleaned(Url::parse("https://example.com/page?keep=1").unwrap())
        );
    }

    #[test]
    fn applies_raw_rules() {
        let clear_urls = test_clear_urls();
        let url = Url::parse("https://amazon.com/dp/123/ref=something?keep=1").unwrap();

        assert_eq!(
            clear_urls.clear(&url).unwrap(),
            ClearUrlResult::Cleaned(Url::parse("https://amazon.com/dp/123?keep=1").unwrap())
        );
    }

    #[test]
    fn redirects_urls() {
        let clear_urls = test_clear_urls();
        let url = Url::parse(
            "https://test.clearurls.xyz/void/index.html?url=https%3A%2F%2Fexample.com%2Fclean",
        )
        .unwrap();

        assert_eq!(
            clear_urls.clear(&url).unwrap(),
            ClearUrlResult::Redirected(Url::parse("https://example.com/clean").unwrap())
        );
    }

    #[test]
    fn blocks_complete_providers() {
        let clear_urls = test_clear_urls();
        let url = Url::parse("https://test.clearurls.xyz/void/block.svg").unwrap();

        assert_eq!(clear_urls.clear(&url).unwrap(), ClearUrlResult::Blocked);
    }

    #[test]
    fn leaves_exception_urls_unchanged() {
        let clear_urls = test_clear_urls();
        let url = Url::parse("https://accounts.google.com/o/oauth2/?utm_source=feed").unwrap();

        assert_eq!(clear_urls.clear(&url).unwrap(), ClearUrlResult::Unchanged);
    }

    fn test_clear_urls() -> ClearUrls {
        ClearUrls::new(
            serde_json::from_str(include_str!("../../../assets/cleanurls_rules.json")).unwrap(),
        )
    }

    #[test]
    fn does_not_reencode_unmodified_query() {
        use super::CompiledRules;
        use regex::Regex;
        use url::Url;

        let rules = CompiledRules {
            rules: vec![Regex::new(r"^utm_").unwrap()],
            raw_rules: vec![],
            referral_marketing: vec![],
            exceptions: vec![],
            redirections: vec![],
        };

        let mut url = Url::parse("https://example.com/?q=hello%20world&flag").unwrap();

        rules.clear_query(&mut url);

        assert_eq!(url.as_str(), "https://example.com/?q=hello%20world&flag");
    }

    #[test]
    fn serializes_to_same_string() {
        let raw_catalog = include_str!("../../../assets/cleanurls_rules.json");
        let catalog: DataCatalog = serde_json::from_str(raw_catalog).unwrap();
        let serialized = serde_json::to_string(&catalog).unwrap();
        let deserialized: DataCatalog = serde_json::from_str(&serialized).unwrap();
        let deserialized_raw = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(serialized, deserialized_raw);
    }
}
