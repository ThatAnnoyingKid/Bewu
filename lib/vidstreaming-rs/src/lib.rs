mod anime_type;
mod client;
mod episode;
mod search_results;
mod util;

pub use self::anime_type::AnimeType;
pub use self::anime_type::FromStrError as ParseAnimeTypeError;
pub use self::client::Client;
pub use self::episode::Episode;
pub use self::episode::FromHtmlError as InvalidEpisodeError;
pub use self::search_results::FromHtmlError as InvalidSearchResultsError;
pub use self::search_results::SearchResults;
use once_cell::sync::Lazy;
use url::Url;

pub(crate) static BASE_URL: Lazy<Url> =
    Lazy::new(|| Url::parse(env!("VIDSTREAMING_RS_BASE_URL")).unwrap());
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

    /// Failed to parse a url
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),

    /// Invalid search results
    #[error("invalid search results")]
    InvalidSearchResults(#[from] InvalidSearchResultsError),

    /// Invalid episode
    #[error("invalid episode")]
    InvalidEpisode(#[from] InvalidEpisodeError),
}
