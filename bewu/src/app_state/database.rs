use anyhow::Context;
use async_rusqlite::rusqlite::named_params;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;
use tracing::error;

const SETUP_SQL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/sql/setup.sql"));
const UPSERT_KITSU_ANIME_SQL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/sql/upsert_kitsu_anime.sql"
));
const UPDATE_KITSU_EPISODES_SQL: &str = "
INSERT OR REPLACE INTO kitsu_episodes (
    episode_id,
    anime_id,
    title,
    synopsis,
    length_minutes,
    number,
    thumbnail_original
) VALUES (
   :episode_id,
   :anime_id,
   :title,
   :synopsis,
   :length_minutes,
   :number,
   :thumbnail_original
);
";

#[derive(Debug, Clone)]
pub struct Anime {
    pub id: NonZeroU64,
    pub slug: String,
    pub synopsis: Option<String>,
    pub title: String,
    pub rating: Option<String>,

    pub poster_large: String,
}

#[derive(Debug, Clone)]
pub struct AnimeEpisode {
    pub episode_id: NonZeroU64,
    pub anime_id: NonZeroU64,

    pub title: Option<String>,
    pub synopsis: Option<String>,
    pub length_minutes: Option<u32>,
    pub number: u32,

    pub thumbnail_original: Option<String>,
}

#[derive(Debug)]
pub struct Database {
    pub(crate) database: async_rusqlite::Database,
}

impl Database {
    pub async fn new<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let database = async_rusqlite::Database::open(path, true, |database| {
            database
                .execute_batch(SETUP_SQL)
                .context("failed to setup database")?;
            Ok(())
        })
        .await?;
        Ok(Self { database })
    }

    /// Upsert kitsu anime
    pub async fn upsert_kitsu_anime(&self, anime: Arc<[Anime]>) -> anyhow::Result<()> {
        self.database
            .access_db(move |database| {
                let transaction = database.transaction()?;
                {
                    let mut statement = transaction.prepare_cached(UPSERT_KITSU_ANIME_SQL)?;
                    for anime in anime.iter() {
                        statement.execute(named_params! {
                            ":id": anime.id.get(),
                            ":slug": anime.slug,
                            ":synopsis": anime.synopsis,
                            ":title": anime.title,
                            ":rating": anime.rating,
                            ":poster_large": anime.poster_large,
                        })?;
                    }
                }
                transaction.commit()?;

                Result::<_, anyhow::Error>::Ok(anime)
            })
            .await??;
        Ok(())
    }

    /// Update kitsu episodes
    pub async fn update_kitsu_episodes(&self, episodes: Arc<[AnimeEpisode]>) -> anyhow::Result<()> {
        self.database
            .access_db(move |database| {
                let transaction = database.transaction()?;
                {
                    let mut statement = transaction.prepare_cached(UPDATE_KITSU_EPISODES_SQL)?;
                    for episode in episodes.iter() {
                        statement.execute(named_params! {
                            ":episode_id": episode.episode_id.get(),
                            ":anime_id": episode.anime_id.get(),
                            ":title": episode.title,
                            ":synopsis": episode.synopsis,
                            ":length_minutes": episode.length_minutes,
                            ":number": episode.number,
                            ":thumbnail_original": episode.thumbnail_original,
                        })?;
                    }
                }
                transaction.commit()?;

                Result::<_, anyhow::Error>::Ok(episodes)
            })
            .await??;
        Ok(())
    }

    /// Shut down the database.
    ///
    /// Should only be called once.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let optmize_result = self
            .database
            .access_db(|database| {
                database.execute("PRAGMA OPTIMIZE;", [])?;
                database.execute("VACUUM;", [])
            })
            .await
            .context("failed to access database")
            .and_then(|v| v.context("failed to execute shutdown commands"))
            .map(|_| ());

        if let Err(error) = optmize_result.as_ref() {
            error!("{error}");
        }

        self.database
            .close()
            .await
            .context("failed to send close command")?;
        let join_result = self
            .database
            .join()
            .await
            .context("failed to join database thread");

        join_result.or(optmize_result)
    }
}
