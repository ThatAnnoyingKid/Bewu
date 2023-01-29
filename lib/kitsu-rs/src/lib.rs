mod anime;
mod episode;

pub use crate::anime::AgeRating;
pub use crate::anime::Anime;
pub use crate::anime::Status;
pub use crate::anime::Subtype;
pub use crate::episode::Episode;
pub use json_api::JsonDocument;
pub use json_api::ResourceObject;
use std::num::NonZeroU64;

/// The error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A JsonApi error
    #[error(transparent)]
    JsonApi(#[from] json_api::Error),
}

/// The client
#[derive(Default, Clone)]
pub struct Client {
    /// The inner json api client
    pub client: json_api::Client,
}

impl Client {
    /// Make a new client
    pub fn new() -> Self {
        Client {
            client: json_api::Client::new(),
        }
    }

    /// Perform a search for anime
    pub async fn search(
        &self,
        query: &str,
    ) -> Result<JsonDocument<Vec<ResourceObject<Anime>>>, Error> {
        let url = format!("https://kitsu.io/api/edge/anime?filter[text]={query}");
        Ok(self.client.get_json_document(&url).await?)
    }

    /// Get an anime
    pub async fn get_anime(
        &self,
        id: NonZeroU64,
    ) -> Result<JsonDocument<ResourceObject<Anime>>, Error> {
        let url = format!("https://kitsu.io/api/edge/anime/{id}");
        Ok(self.client.get_json_document(&url).await?)
    }

    /// Get anime epsiodes
    pub async fn get_anime_episodes(
        &self,
        anime_id: NonZeroU64,
    ) -> Result<JsonDocument<Vec<ResourceObject<Episode>>>, Error> {
        let url = format!("https://kitsu.io/api/edge/anime/{anime_id}/episodes");
        Ok(self.client.get_json_document(&url).await?)
    }

    /// Get an episode
    pub async fn get_episode(
        &self,
        episode_id: NonZeroU64,
    ) -> Result<JsonDocument<ResourceObject<Episode>>, Error> {
        let url = format!("https://kitsu.io/api/edge/episodes/{episode_id}");
        Ok(self.client.get_json_document(&url).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCHES: &[&str] = &[
        "3-gatsu no Lion 2nd Season",
        "cowboy bebop",
        "5 Centimeter per Second",
        "food",
        "high",
        "hello",
    ];

    const ANIME: &[&str] = &[
        include_str!("../test_data/anime/46174.json"),
        include_str!("../test_data/anime/13401.json"),
        include_str!("../test_data/anime/5.json"),
    ];

    const EPISODES: &[&str] = &[
        include_str!("../test_data/episodes/99605.json"),
        include_str!("../test_data/episodes/89.json"),
    ];

    #[test]
    fn parse_searches() {
        for search in SEARCHES {
            let path = format!("test_data/searches/{search}.json");
            let search_json = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("failed to read \"{path}\": {}", e);
            });
            let search_result =
                serde_json::from_str::<JsonDocument<Vec<ResourceObject<Anime>>>>(&search_json);

            match search_result {
                Ok(_search) => {
                    // TODO: Consider comparing with "Answer" array.
                }
                Err(e) => {
                    panic!("failed to parse \"{search}\": {e:#?}");
                }
            }
        }
    }

    #[test]
    fn parse_anime() {
        for anime in ANIME {
            let _anime = serde_json::from_str::<JsonDocument<ResourceObject<Anime>>>(anime)
                .expect("failed to parse");
        }
    }

    #[test]
    fn parse_episode() {
        for episode in EPISODES {
            let _episode = serde_json::from_str::<JsonDocument<ResourceObject<Episode>>>(episode)
                .expect("failed to parse");
        }
    }

    #[tokio::test]
    async fn it_works() {
        let client = Client::new();
        let search_result = client.search("food").await.expect("failed to search");
        dbg!(&search_result);
        let anime_id = NonZeroU64::new(1).unwrap();
        let anime = client
            .get_anime(anime_id)
            .await
            .expect("failed to get anime");
        dbg!(&anime);
        let episodes = client
            .get_anime_episodes(anime_id)
            .await
            .expect("failed to get anime episodes");
        dbg!(&episodes);

        // First id determined experimentally
        let episode_id = NonZeroU64::new(27).unwrap();
        let episode = client
            .get_episode(episode_id)
            .await
            .expect("failed to get episode");

        dbg!(&episode);
    }
}
