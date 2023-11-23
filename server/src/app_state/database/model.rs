use std::num::NonZeroU64;

/// Anime data fetched from kitsu
#[derive(Debug, Clone)]
pub struct KitsuAnime {
    /// The unique id
    pub id: NonZeroU64,

    /// The url slug
    pub slug: String,

    /// The synopsis
    pub synopsis: Option<String>,

    /// The title
    pub title: String,

    /// The rating
    ///
    /// This is a stringified float from 0.00-100.00.
    /// It has 2 decimal places.
    /// It is stored as a string as that is what the api returns,
    /// and a loss of precision can be avoided by keeping it as a string.
    pub rating: Option<String>,

    pub poster_large: String,

    /// The timestamp of the last update.
    ///
    /// This is the number of seconds from the unix epoch.
    pub last_update: u64,
}

/// Anime episode data fetched from kitsu
#[derive(Debug, Clone)]
pub struct KitsuAnimeEpisode {
    /// The episode id
    pub episode_id: NonZeroU64,

    /// The anime id
    pub anime_id: NonZeroU64,

    /// The title
    pub title: Option<String>,

    /// The synopsis
    pub synopsis: Option<String>,

    /// The length, in minutes
    pub length_minutes: Option<u32>,

    /// The episode number, in the current season.
    pub number: u32,

    /// The original thumbnail url.
    pub thumbnail_original: Option<String>,

    /// The timestamp of the last update.
    ///
    /// This is the number of seconds from the unix epoch.
    pub last_update: u64,
}
