use crate::BASE_URL;
use once_cell::sync::Lazy;
use scraper::ElementRef;
use scraper::Html;
use scraper::Selector;
use url::Url;

static LISTING_ITEMS_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".listing.items li").unwrap());

static NAME_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse(".name").unwrap());
static A_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("a").unwrap());

/// Error that may occur while parsing search results
#[derive(thiserror::Error, Debug)]
pub enum FromHtmlError {
    /// Missing results
    #[error("missing search results")]
    MissingResults,

    /// Invalid entry
    #[error("invalid entry")]
    InvalidEntry(#[from] FromElementError),
}

/// Search results
#[derive(Debug)]
pub struct SearchResults {
    /// Entries in this result
    pub entries: Vec<SearchEntry>,
}

impl SearchResults {
    /// Try to get a [`SearchResults`] from Html.
    pub(crate) fn from_html(html: &Html) -> Result<Self, FromHtmlError> {
        let entries = html
            .select(&LISTING_ITEMS_SELECTOR)
            .map(SearchEntry::from_element)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SearchResults { entries })
    }
}

/// Search Entry
#[derive(Debug)]
pub struct SearchEntry {
    /// Entry name
    pub name: String,

    /// Entry Url
    pub url: Url,
}

/// Error that may occur while parsing a search entry
#[derive(thiserror::Error, Debug)]
pub enum FromElementError {
    /// Missing name
    #[error("missing name")]
    MissingName,

    /// Missing url
    #[error("missing url")]
    MissingUrl,

    /// Invalid url
    #[error("invalid url")]
    InvalidUrl(#[source] url::ParseError),
}

impl SearchEntry {
    /// Try to parse a search entry from an element
    pub(crate) fn from_element(el: ElementRef) -> Result<Self, FromElementError> {
        let name = el
            .select(&NAME_SELECTOR)
            .next()
            .and_then(|el| el.text().next())
            .ok_or(FromElementError::MissingName)?
            .trim()
            .to_string();

        let relative_url = el
            .select(&A_SELECTOR)
            .next()
            .and_then(|el| el.value().attr("href"))
            .ok_or(FromElementError::MissingUrl)?;

        let url = BASE_URL
            .join(relative_url)
            .map_err(FromElementError::InvalidUrl)?;

        Ok(SearchEntry { name, url })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const SEARCH_BLEACH: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/searches/bleach.html"
    ));

    #[test]
    fn parse_search_bleach() {
        let doc = Html::parse_document(SEARCH_BLEACH);

        let res = SearchResults::from_html(&doc).expect("failed to parse search results");
        assert!(!res.entries.is_empty());
    }
}
