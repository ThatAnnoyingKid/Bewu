use crate::EncryptedVideoData;
use crate::Episode;
use crate::Error;
use crate::SearchResults;
use crate::VideoData;
use crate::VideoPlayer;
use crate::SEARCH_URL;
use scraper::Html;
use std::num::NonZeroU32;
use url::Url;

pub(crate) const USER_AGENT_VALUE: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/103.0.5015.0 Safari/537.36";

/// The vidstreaming client
#[derive(Debug, Clone)]
pub struct Client {
    /// The inner http client
    pub client: reqwest::Client,
}

impl Client {
    /// Make a new client
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT_VALUE)
            .build()
            .expect("failed to build client");
        Self { client }
    }

    /// Get the url as html, then transform it
    async fn get_html<F, T>(&self, url: &str, transform: F) -> Result<T, Error>
    where
        F: FnOnce(Html) -> T + Send + 'static,
        T: Send + 'static,
    {
        let response = self.client.get(url).send().await?.error_for_status()?;
        let text = response.text().await?;
        Ok(tokio::task::spawn_blocking(move || {
            let html = Html::parse_document(&text);
            transform(html)
        })
        .await?)
    }

    /// Search with the given query and page number.
    ///
    /// `page` starts at 1.
    pub async fn search(&self, query: &str, page: NonZeroU32) -> Result<SearchResults, Error> {
        let url = Url::parse_with_params(
            SEARCH_URL,
            &[
                ("keyword", query),
                ("page", itoa::Buffer::new().format(page.get())),
            ],
        )?;
        let results = self
            .get_html(url.as_str(), |html| SearchResults::from_html(&html))
            .await??;
        Ok(results)
    }

    /// Get an episode by url
    pub async fn get_episode(&self, url: &str) -> Result<Episode, Error> {
        Ok(self
            .get_html(url, |html| Episode::from_html(&html))
            .await??)
    }

    /// Get an episode's video player by url
    pub async fn get_video_player(&self, url: &str) -> Result<VideoPlayer, Error> {
        Ok(self
            .get_html(url, |html| VideoPlayer::from_html(&html))
            .await??)
    }

    /// Get the video data for a given video player
    pub async fn get_video_player_video_data(
        &self,
        player: &VideoPlayer,
    ) -> Result<VideoData, Error> {
        let url = player.generate_video_data_url()?;
        let encrypted_video_data: EncryptedVideoData = self
            .client
            .get(url.as_str())
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let video_data = encrypted_video_data.decrypt(player)?;
        Ok(video_data)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn search() {
        let client = Client::new();
        let res = client
            .search("bleach", NonZeroU32::new(1).unwrap())
            .await
            .expect("failed to search");

        dbg!(&res);
        assert!(!res.entries.is_empty());
    }

    #[tokio::test]
    async fn get_episode() {
        let urls = [
            "https://gogo-stream.com/videos/bleach-episode-366",
            "https://anihdplay.com/videos/black-clover-tv-dub-episode-170",
        ];
        let client = Client::new();
        for url in urls {
            let episode = client
                .get_episode(url)
                .await
                .expect("failed to get episode");

            dbg!(&episode);
            assert!(!episode.related_episodes.is_empty());
        }
    }

    #[tokio::test]
    async fn get_video_player() {
        let client = Client::new();
        let url = "https://gogo-stream.com/videos/bleach-episode-366";
        let episode = client
            .get_episode(url)
            .await
            .expect("failed to get episode");

        assert!(!episode.related_episodes.is_empty());

        let player = client
            .get_video_player(episode.video_player_url.as_str())
            .await
            .expect("failed to get player");

        dbg!(&player);

        let video_data = client
            .get_video_player_video_data(&player)
            .await
            .expect("failed to get video data");
        dbg!(&video_data);
    }
}
