use anyhow::Context;
use log::warn;
use regex::Regex;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use url::{form_urlencoded, Url};

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClearUrlResult {
    Unchanged,
    Cleaned(Url),
    Redirected(Url),
    Blocked,
}

#[derive(Deserialize)]
pub struct DataCatalog {
    pub providers: BTreeMap<String, Provider>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    #[serde(deserialize_with = "deserialize_regex")]
    pub url_pattern: Regex,
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
            .filter(move |provider| provider.url_pattern.is_match(url))
    }
}

impl Provider {
    fn rules(&self) -> anyhow::Result<&CompiledRules> {
        self.compiled_rules
            .get_or_init(|| compile_rules(self).map_err(|err| err.to_string()))
            .as_ref()
            .map_err(|err| anyhow::anyhow!("{err}"))
    }
}

#[derive(Debug)]
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

fn deserialize_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let pattern = String::deserialize(deserializer)?;
    Regex::new(&pattern).map_err(de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::{ClearUrlResult, ClearUrls, DataCatalog};
    use url::Url;

    #[test]
    fn deserializes_clearurls_data_catalog() {
        let catalog: DataCatalog = serde_json::from_str(include_str!("test_rules.json")).unwrap();

        let amazon = catalog.provider("amazon").unwrap();
        assert_eq!(
            amazon.url_pattern.as_str(),
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
        let catalog: DataCatalog = serde_json::from_str(include_str!("test_rules.json")).unwrap();

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
        ClearUrls::new(serde_json::from_str(include_str!("test_rules.json")).unwrap())
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
}
