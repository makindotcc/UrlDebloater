use tracing::{debug, error};
use url::Url;

use crate::UrlWasher;

#[derive(Default)]
pub struct TextWasher {
    url_washer: UrlWasher,
}

impl TextWasher {
    pub async fn wash(&self, text: &str) -> String {
        let mut original_separators = Vec::new();
        let wash_tasks = text
            .split(|c: char| {
                let is_whitespace = c.is_whitespace();
                if is_whitespace {
                    original_separators.push(c);
                }
                is_whitespace
            })
            .map(|part| async move {
                if !part.starts_with("http://") && !part.starts_with("https://") {
                    return part.to_string();
                }
                let url = match Url::parse(part) {
                    Ok(url) => url,
                    Err(_) => return part.to_string(),
                };
                debug!("Washing part of text: {url}");
                match self.url_washer.wash(&url).await {
                    Ok(Some(clean_url)) => clean_url.to_string(),
                    Ok(None) => part.to_string(),
                    Err(err) => {
                        error!("Could not wash url '{}': {:?}", part, err);
                        part.to_string()
                    }
                }
            })
            .collect::<Vec<_>>();
        let mut patched = String::new();
        for (index, task) in wash_tasks.into_iter().enumerate() {
            patched.push_str(&task.await);
            if let Some(separator) = original_separators.get(index) {
                patched.push(*separator);
            }
        }
        patched
    }
}

#[cfg(test)]
mod tests {
    use super::TextWasher;

    #[tokio::test]
    pub async fn test() {
        let text_washer = TextWasher::default();
        let cleaned = text_washer.wash("text").await;
        assert_eq!("", cleaned);
    }
}
