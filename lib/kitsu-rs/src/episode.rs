use std::collections::HashMap;
use url::Url;

/// An epsiode from an Anime
#[derive(Debug, serde::Deserialize)]
pub struct Episode {
    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    /// Titles?
    pub titles: HashMap<String, Option<String>>,

    /// The canonical title
    #[serde(rename = "canonicalTitle")]
    pub canonical_title: Option<String>,

    /// The season number
    #[serde(rename = "seasonNumber")]
    pub season_number: Option<u32>,

    /// ?
    pub number: u32,

    /// ?
    #[serde(rename = "relativeNumber")]
    pub relative_number: Option<u32>,

    /// Episode synopsis
    pub synopsis: Option<String>,

    /// The date the epsiode was aired
    pub airdate: Option<String>,

    /// The length of the episode in minutes
    pub length: Option<u32>,

    /// Thumbnail data
    pub thumbnail: Option<Thumbnail>,
}

/// Thumbnail data
#[derive(Debug, serde::Deserialize)]
pub struct Thumbnail {
    /// Original thumbnail url
    pub original: Url,
}
