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
    pub canonical_title: String,
    
    /// The season number
    #[serde(rename = "seasonNumber")]
    pub season_number: u32,
    
    /// ?
    pub number: u32,
    
    /// ?
    #[serde(rename = "relativeNumber")]
    pub relative_number: Option<u32>,
    
    /// Episode synopsis
    pub synopsis: String,
    
    /// The date the epsiode was aired
    pub airdate: String,
    
    /// The length of the episode in minutes
    pub length: u32,
    
    /// Thumbnail data
    pub thumbnail: Thumbnail,
}

/// Thumbnail data
#[derive(Debug, serde::Deserialize)]
pub struct Thumbnail {
    /// Original thumbnail url
    pub original: Url,
}