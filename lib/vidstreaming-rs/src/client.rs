use crate::Episode;
use crate::Error;
use crate::SearchResults;
use crate::SEARCH_URL;
use scraper::Html;
use std::num::NonZeroU32;
use url::Url;

/// The vidstreaming client
#[derive(Debug, Clone)]
pub struct Client {
    /// The inner http client
    pub client: reqwest::Client,
}

impl Client {
    /// Make a new client
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Get the url as html, then transform it
    async fn get_html<F, T>(&self, url: &str, transform: F) -> Result<T, Error>
    where
        F: FnOnce(Html) -> T + Send + 'static,
        T: Send + 'static,
    {
        let text = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(tokio::task::spawn_blocking(move || {
            let html = Html::parse_document(&text);
            transform(html)
        })
        .await?)
    }

    /// Search with the given query and page number.
    ///
    /// `page` starts at 1.
    pub async fn search(&self, query: &str, page: NonZeroU32) -> Result<SearchResults, Error> {
        let url = Url::parse_with_params(
            SEARCH_URL,
            &[
                ("keyword", query),
                ("page", itoa::Buffer::new().format(page.get())),
            ],
        )?;
        let results = self
            .get_html(url.as_str(), |html| SearchResults::from_html(&html))
            .await??;
        Ok(results)
    }

    /// Get an episode by url
    pub async fn get_episode(&self, url: &str) -> Result<Episode, Error> {
        Ok(self
            .get_html(url, |html| Episode::from_html(&html))
            .await??)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn search() {
        let client = Client::new();
        let res = client
            .search("bleach", NonZeroU32::new(1).unwrap())
            .await
            .expect("failed to search");

        dbg!(&res);
        assert!(!res.entries.is_empty());
    }
}
