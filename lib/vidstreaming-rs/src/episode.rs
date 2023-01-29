use crate::AnimeType;
use crate::ParseAnimeTypeError;
use crate::BASE_URL;
use once_cell::sync::Lazy;
use scraper::ElementRef;
use scraper::Html;
use scraper::Selector;
use std::borrow::Cow;
use url::Url;

// Episode Selectors
static VIDEO_INFO_TITLE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".video-info h1").unwrap());
static VIDEO_INFO_DESCRIPTION_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".video-info .video-details .post-entry").unwrap());
static LINK_URL_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("link[rel=\"canonical\"]").unwrap());
static VIDEO_INFO_IFRAME_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".video-info iframe").unwrap());
static VIDEO_INFO_RELATED_EPISODES_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".video-info .listing.items.lists .video-block").unwrap());
static A_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("a").unwrap());

// RelatedEpisode Selectors
static NAME_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse(".name").unwrap());
static TYPE_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse(".type span").unwrap());

/// Error that may occur while parsing an episode from a doc
#[derive(thiserror::Error, Debug)]
pub enum FromHtmlError {
    /// Missing name
    #[error("missing name")]
    MissingName,

    /// Missing video player url
    #[error("missing video player url")]
    MissingVideoPlayerUrl,

    /// Missing url
    #[error("missing url")]
    MissingUrl,

    /// Invalid url
    #[error("invalid url")]
    InvalidUrl(#[from] url::ParseError),

    /// Invalid related episode
    #[error("invalid related episode")]
    InvalidRelatedEpisode(#[from] FromElementError),
}

/// An episode
#[derive(Debug)]
pub struct Episode {
    /// Episode name
    pub name: String,

    /// Episode description
    pub description: Option<String>,

    /// Episode Url
    pub url: Url,

    /// Video Player url for episode
    pub video_player_url: Url,

    /// Related episode urls
    pub related_episodes: Vec<RelatedEpisode>,
}

impl Episode {
    /// Parse an [`Episode`] from Html
    pub(crate) fn from_html(html: &Html) -> Result<Self, FromHtmlError> {
        let name = html
            .select(&VIDEO_INFO_TITLE_SELECTOR)
            .next()
            .and_then(|el| el.text().next())
            .ok_or(FromHtmlError::MissingName)?
            .trim()
            .to_string();

        let description = html
            .select(&VIDEO_INFO_DESCRIPTION_SELECTOR)
            .next()
            .and_then(|el| el.text().map(|t| t.trim()).find(|t| !t.is_empty()))
            .map(|description| description.to_string());

        // <link rel="canonical" href="/videos/yi-nian-yong-heng-episode-51"/>
        let url = html
            .select(&LINK_URL_SELECTOR)
            .next()
            .and_then(|el| {
                let href = el.value().attr("href")?;
                Some(BASE_URL.join(href))
            })
            .ok_or(FromHtmlError::MissingUrl)??;

        let video_player_url = html
            .select(&VIDEO_INFO_IFRAME_SELECTOR)
            .next()
            .and_then(|el| {
                let src = crate::util::make_https(el.value().attr("src")?);
                Some(Url::parse(&src))
            })
            .ok_or(FromHtmlError::MissingVideoPlayerUrl)??;

        let mut related_episodes = html
            .select(&VIDEO_INFO_RELATED_EPISODES_SELECTOR)
            .map(RelatedEpisode::from_element)
            .collect::<Result<Vec<_>, _>>()?;
        related_episodes.reverse();

        Ok(Episode {
            name,
            description,
            url,
            video_player_url,
            related_episodes,
        })
    }

    /// Get the title of the series.
    pub fn series_title(&self) -> Option<Cow<'_, str>> {
        self.video_player_url.query_pairs().find_map(
            |(k, v)| {
                if k == "title" {
                    Some(v)
                } else {
                    None
                }
            },
        )
    }

    /// Get the id of the episode.
    pub fn get_id(&self) -> Option<Cow<'_, str>> {
        self.video_player_url.query_pairs().find_map(
            |(k, v)| {
                if k == "id" {
                    Some(v)
                } else {
                    None
                }
            },
        )
    }

    /*
    /// Get the download url of the video
    pub fn get_download_data_url(&self) -> Option<Url> {
        Url::parse(&format!(
            "{}/download?id={}",
            crate::BASE_URL,
            self.get_id()?
        ))
        .ok()
    }
    */
}

/// Error from extracting a [`RelatedEpisode`] from a html element.
#[derive(thiserror::Error, Debug)]
pub enum FromElementError {
    /// Missing Name
    #[error("missing name")]
    MissingName,

    /// Missing url
    #[error("missing url")]
    MissingUrl,

    /// Missing anime type
    #[error("missing anime type")]
    MissingAnimeType,

    /// Invalid url
    #[error("invalid url")]
    InvalidUrl(#[from] url::ParseError),

    /// Invalid anime type
    #[error("invalid anime type")]
    InvalidAnimeType(ParseAnimeTypeError),
}

/// Related Episode
#[derive(Debug)]
pub struct RelatedEpisode {
    /// Name of episode
    pub name: String,

    /// Url of episode
    pub url: Url,

    /// Type of episode
    pub anime_type: AnimeType,
}

impl RelatedEpisode {
    /// Make a [`RelatedEpisode`] from an `ElementRef`
    fn from_element(el: ElementRef) -> Result<Self, FromElementError> {
        let name = el
            .select(&NAME_SELECTOR)
            .next()
            .and_then(|el| el.text().next())
            .ok_or(FromElementError::MissingName)?
            .trim()
            .to_string();

        let link = el
            .select(&A_SELECTOR)
            .next()
            .and_then(|el| el.value().attr("href"))
            .ok_or(FromElementError::MissingUrl)?;

        let url = BASE_URL.join(link)?;

        let anime_type = el
            .select(&TYPE_SELECTOR)
            .next()
            .and_then(|el| el.text().map(|t| t.trim()).find(|t| !t.is_empty()))
            .ok_or(FromElementError::MissingAnimeType)?
            .parse()
            .map_err(FromElementError::InvalidAnimeType)?;

        Ok(Self {
            name,
            url,
            anime_type,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const BLEACH_EPISODE_366: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/episodes/bleach-366.html"
    ));

    #[test]
    fn parse_bleach_366() {
        let html = Html::parse_document(BLEACH_EPISODE_366);
        let res = Episode::from_html(&html).expect("failed to parse episode");

        dbg!(&res);
        assert!(!res.related_episodes.is_empty());
    }
}
