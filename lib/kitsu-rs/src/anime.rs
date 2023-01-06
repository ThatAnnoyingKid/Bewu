use std::collections::HashMap;
use url::Url;

/// An Anime Object.
/// [Spec](https://kitsu.docs.apiary.io/#reference/anime)
#[derive(Debug, serde::Deserialize)]
pub struct Anime {
    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    /// The URL slug
    pub slug: String,

    /// The synopsis
    pub synopsis: Option<String>,

    /// Titles?
    pub titles: HashMap<String, Option<String>>,

    /// The canonical title
    #[serde(rename = "canonicalTitle")]
    pub canonical_title: String,

    #[serde(rename = "abbreviatedTitles")]
    pub abbreviated_titles: Option<Vec<String>>,

    /// The average rating.
    ///
    /// This is a stringified float from 0-100.
    #[serde(rename = "averageRating")]
    pub average_rating: Option<String>,

    #[serde(rename = "ratingFrequencies")]
    pub rating_frequencies: HashMap<u64, String>,

    #[serde(rename = "userCount")]
    pub user_count: u64,

    #[serde(rename = "favoritesCount")]
    pub favorites_count: u64,

    #[serde(rename = "startDate")]
    pub start_date: Option<String>,

    #[serde(rename = "endDate")]
    pub end_date: Option<String>,

    #[serde(rename = "popularityRank")]
    pub popularity_rank: u64,

    #[serde(rename = "ratingRank")]
    pub rating_rank: Option<u64>,

    #[serde(rename = "ageRating")]
    pub age_rating: Option<AgeRating>,

    #[serde(rename = "ageRatingGuide")]
    pub age_rating_guide: Option<String>,

    pub subtype: Subtype,
    pub status: Status,
    pub tba: Option<String>,

    /// Poster image sizes and urls
    #[serde(rename = "posterImage")]
    pub poster_image: Images,

    /// Cover image sizes and urls
    #[serde(rename = "coverImage")]
    pub cover_image: Option<Images>,

    /// The number of episodes
    #[serde(rename = "episodeCount")]
    pub episode_count: Option<u64>,

    #[serde(rename = "episodeLength")]
    pub episode_length: Option<u64>,

    #[serde(rename = "youtubeVideoId")]
    pub youtube_video_id: Option<String>,

    /// Whether this is nsfw
    pub nsfw: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(from = "&str")]
pub enum AgeRating {
    General,
    ParentalGuidance,
    Restricted,
    Explicit,

    Other(String),
}

impl<'a> From<&'a str> for AgeRating {
    fn from(s: &str) -> Self {
        match s {
            "G" => Self::General,
            "PG" => Self::ParentalGuidance,
            "R" => Self::Restricted,
            "R18" => Self::Explicit,
            _ => Self::Other(s.into()),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(from = "&str")]
pub enum Subtype {
    Ona,
    Ova,
    Tv,
    Movie,
    Music,
    Special,

    Other(String),
}

impl<'a> From<&'a str> for Subtype {
    fn from(s: &'a str) -> Self {
        match s {
            "ONA" => Self::Ona,
            "OVA" => Self::Ova,
            "TV" => Self::Tv,
            "movie" => Self::Movie,
            "music" => Self::Music,
            "special" => Self::Special,

            _ => Self::Other(s.into()),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(from = "&str")]
pub enum Status {
    Current,
    Finished,
    Tba,
    Unreleased,
    Upcoming,

    Other(String),
}

impl<'a> From<&'a str> for Status {
    fn from(s: &'a str) -> Self {
        match s {
            "current" => Self::Current,
            "finished" => Self::Finished,
            "tba" => Self::Tba,
            "unreleased" => Self::Unreleased,
            "upcoming" => Self::Upcoming,
            _ => Self::Other(s.into()),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct Images {
    /// The tiny image url
    pub tiny: Url,

    /// The small image url
    pub small: Url,

    /// The medium image url
    pub medium: Option<Url>,

    /// The large image url
    pub large: Url,

    /// The original image url
    pub original: Url,

    /// Metadata?
    pub meta: ImagesMetadata,
}

#[derive(Debug, serde::Deserialize)]
pub struct ImagesMetadata {
    /// Image dimensions
    pub dimensions: ImagesMetadataDimensions,
}

#[derive(Debug, serde::Deserialize)]
pub struct ImagesMetadataDimensions {
    pub tiny: ImageDimension,
    pub small: ImageDimension,
    pub medium: Option<ImageDimension>,
    pub large: ImageDimension,
}

/// Image dimensions
#[derive(Debug, serde::Deserialize)]
pub struct ImageDimension {
    /// Image width
    pub width: Option<u32>,

    /// Image height
    pub height: Option<u32>,
}
