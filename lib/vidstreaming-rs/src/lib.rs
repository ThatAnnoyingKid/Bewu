mod client;
mod search_results;

pub use self::client::Client;
pub use self::search_results::FromHtmlError as InvalidSearchResultsError;
pub use self::search_results::SearchResults;

pub(crate) const BASE_URL: &str = env!("VIDSTREAMING_RS_BASE_URL");
pub(crate) const SEARCH_URL: &str = concat!(env!("VIDSTREAMING_RS_BASE_URL"), "search.html");

/// The library error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A HTTP error
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    /// A tokio join error
    #[error(transparent)]
    TokioJoin(#[from] tokio::task::JoinError),

    /// Invalid search results
    #[error("invalid search results")]
    InvalidSearchResults(#[from] InvalidSearchResultsError),

    /// Failed to parse a url
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),
}
