use std::collections::HashMap;

/// An Anime Object.
/// [Spec](https://kitsu.docs.apiary.io/#reference/anime)
#[derive(Debug, serde::Deserialize)]
pub struct Anime {
    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub slug: String,
    pub synopsis: String,
    pub titles: HashMap<String, Option<String>>,

    #[serde(rename = "canonicalTitle")]
    pub canonical_title: String,

    #[serde(rename = "abbreviatedTitles")]
    pub abbreviated_titles: Option<Vec<String>>,

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

    #[serde(rename = "posterImage")]
    pub poster_image: serde_json::Value,

    #[serde(rename = "coverImage")]
    pub cover_image: Option<serde_json::Value>,

    #[serde(rename = "episodeCount")]
    pub episode_count: Option<u64>,

    #[serde(rename = "episodeLength")]
    pub episode_length: Option<u64>,

    #[serde(rename = "youtubeVideoId")]
    pub youtube_video_id: Option<String>,

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
