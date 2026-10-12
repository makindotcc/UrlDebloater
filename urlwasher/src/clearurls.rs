use regex::Regex;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use std::collections::BTreeMap;
use url::{form_urlencoded, Url};

pub struct ClearUrls {
    catalog: DataCatalog,
}

impl ClearUrls {
    pub fn new(catalog: DataCatalog) -> Self {
        Self { catalog }
    }

    pub fn clear(&self, url: &Url) -> Result<ClearUrlResult, url::ParseError> {
        let mut current = url.clone();
        let mut changed = false;

        for provider in self.catalog.matching_providers(url) {
            if provider.is_exception(url.as_str()) {
                continue;
            }
            if provider.complete_provider {
                return Ok(ClearUrlResult::Blocked);
            }
            if let Some(redirected) = provider.redirect(url)? {
                return Ok(ClearUrlResult::Redirected(redirected));
            }
            changed |= provider.clear_query(&mut current);
            changed |= provider.apply_raw_rules(&mut current)?;
        }

        if changed {
            Ok(ClearUrlResult::Cleaned(current))
        } else {
            Ok(ClearUrlResult::Unchanged)
        }
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    #[serde(deserialize_with = "deserialize_regex")]
    pub url_pattern: Regex,
    #[serde(default)]
    pub complete_provider: bool,
    #[serde(default, deserialize_with = "deserialize_regex_vec")]
    pub rules: Vec<Regex>,
    #[serde(default, deserialize_with = "deserialize_regex_vec")]
    pub raw_rules: Vec<Regex>,
    #[serde(default, deserialize_with = "deserialize_regex_vec")]
    pub referral_marketing: Vec<Regex>,
    #[serde(default, deserialize_with = "deserialize_regex_vec")]
    pub exceptions: Vec<Regex>,
    #[serde(default, deserialize_with = "deserialize_regex_vec")]
    pub redirections: Vec<Regex>,
    #[serde(default)]
    pub force_redirection: bool,
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
    fn is_exception(&self, url: &str) -> bool {
        self.exceptions
            .iter()
            .any(|exception| exception.is_match(url))
    }

    fn redirect(&self, url: &Url) -> Result<Option<Url>, url::ParseError> {
        for redirection in &self.redirections {
            let Some(captures) = redirection.captures(url.as_str()) else {
                continue;
            };
            let Some(target) = captures.get(1) else {
                continue;
            };
            let target = decode_url_component(target.as_str());
            return Url::parse(&target).map(Some);
        }
        Ok(None)
    }

    fn clear_query(&self, url: &mut Url) -> bool {
        let Some(_) = url.query() else {
            return false;
        };

        let kept_pairs = url
            .query_pairs()
            .filter(|(key, _)| !self.should_remove_query_param(key))
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();

        let original_query = url.query().map(str::to_owned);
        url.set_query(None);
        if !kept_pairs.is_empty() {
            let mut query_pairs = url.query_pairs_mut();
            for (key, value) in kept_pairs {
                query_pairs.append_pair(&key, &value);
            }
        }

        url.query() != original_query.as_deref()
    }

    fn should_remove_query_param(&self, key: &str) -> bool {
        self.rules.iter().any(|rule| rule.is_match(key))
            || self
                .referral_marketing
                .iter()
                .any(|rule| rule.is_match(key))
    }

    fn apply_raw_rules(&self, url: &mut Url) -> Result<bool, url::ParseError> {
        let mut raw_url = url.as_str().to_owned();
        let original = raw_url.clone();
        for raw_rule in &self.raw_rules {
            raw_url = raw_rule.replace_all(&raw_url, "").into_owned();
        }
        if raw_url == original {
            return Ok(false);
        }
        *url = Url::parse(&raw_url)?;
        Ok(true)
    }
}

fn decode_url_component(value: &str) -> String {
    form_urlencoded::parse(value.as_bytes())
        .map(|(decoded, _)| decoded.into_owned())
        .next()
        .unwrap_or_else(|| value.to_owned())
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let pattern = String::deserialize(deserializer)?;
    Regex::new(&pattern).map_err(de::Error::custom)
}

fn deserialize_regex_vec<'de, D>(deserializer: D) -> Result<Vec<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .map(|pattern| Regex::new(&pattern).map_err(de::Error::custom))
        .collect()
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
        assert!(amazon.rules.iter().any(|rule| rule.as_str() == "qid"));
        assert!(amazon
            .raw_rules
            .iter()
            .any(|rule| rule.as_str() == "\\/ref=[^/?]*"));
        assert!(amazon
            .referral_marketing
            .iter()
            .any(|rule| rule.as_str() == "tag"));
        assert!(!amazon.force_redirection);

        let blocked_provider = catalog.provider("fls-na.amazon").unwrap();
        assert!(blocked_provider.complete_provider);
        assert!(blocked_provider.rules.is_empty());

        let google = catalog.provider("google").unwrap();
        assert!(google.force_redirection);
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
}
