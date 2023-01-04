mod types;

pub use crate::types::AgeRating;
pub use crate::types::Anime;
pub use crate::types::Status;
pub use crate::types::Subtype;
pub use json_api::JsonDocument;
pub use json_api::ResourceObject;

/// The error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A JsonApi error
    #[error(transparent)]
    JsonApi(#[from] json_api::Error),
}

/// The client
#[derive(Default)]
pub struct Client {
    client: json_api::Client,
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
        let url = format!("https://kitsu.io/api/edge/anime?filter[text]={}", query);
        Ok(self.client.get_json_document(&url).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCHES: &[&str] = &[
        include_str!("../test_data/3-gatsu_no_Lion_2nd_Season_search.json"),
        include_str!("../test_data/cowboy_bebop_search.json"),
        include_str!("../test_data/5_Centimeter_per_Second_search.json"),
        include_str!("../test_data/food_search.json"),
        include_str!("../test_data/high_search.json"),
    ];

    const ANIME: &[&str] = &[include_str!("../test_data/anime/46174.json")];

    #[test]
    fn parse_searches() {
        for search_json in SEARCHES {
            let search =
                serde_json::from_str::<JsonDocument<Vec<ResourceObject<Anime>>>>(search_json);

            match search {
                Ok(_search) => {
                    // TODO: Consider comparing with "Answer" array.
                }
                Err(e) => {
                    panic!("{:#?}", e);
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

    #[tokio::test]
    async fn it_works() {
        let client = Client::new();
        let res = client.search("food").await.unwrap();
        dbg!(res);
    }
}
