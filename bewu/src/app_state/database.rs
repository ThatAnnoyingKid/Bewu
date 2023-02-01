mod model;

pub use self::model::KitsuAnime;
pub use self::model::KitsuAnimeEpisode;
use anyhow::Context;
use async_rusqlite::rusqlite::named_params;
use async_rusqlite::rusqlite::OptionalExtension;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;
use tracing::error;

pub trait AsSlice<T> {
    fn as_slice(&self) -> &[T];
}

impl<T> AsSlice<T> for T {
    fn as_slice(&self) -> &[T] {
        std::slice::from_ref(self)
    }
}

impl<T> AsSlice<T> for &[T] {
    fn as_slice(&self) -> &[T] {
        self
    }
}

impl<T> AsSlice<T> for Arc<T> {
    fn as_slice(&self) -> &[T] {
        std::slice::from_ref(self)
    }
}

impl<T> AsSlice<T> for Arc<[T]> {
    fn as_slice(&self) -> &[T] {
        self
    }
}

const SETUP_SQL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/sql/setup.sql"));
const UPSERT_KITSU_ANIME_SQL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/sql/upsert_kitsu_anime.sql"
));
const UPDATE_KITSU_EPISODE_SQL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/sql/upsert_kitsu_episode.sql"
));

const GET_KITSU_ANIME_SQL: &str = "
SELECT 
    id, 
    slug,
    synopsis,
    title,
    rating,
    poster_large,
    last_update
FROM
    kitsu_anime
WHERE 
    id = :id;
";

#[derive(Debug, Clone)]
pub struct Database {
    pub(crate) database: async_rusqlite::Database,
}

impl Database {
    /// Create a new database at the given path, or open it if it already exists.
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

    /// Upsert kitsu anime.
    pub async fn upsert_kitsu_anime<A>(&self, anime: A) -> anyhow::Result<()>
    where
        A: AsSlice<KitsuAnime> + Send + 'static,
    {
        self.database
            .access_db(move |database| {
                let transaction = database.transaction()?;
                {
                    let mut statement = transaction.prepare_cached(UPSERT_KITSU_ANIME_SQL)?;
                    let anime = anime.as_slice();
                    for anime in anime.iter() {
                        statement.execute(named_params! {
                            ":id": anime.id.get(),
                            ":slug": anime.slug,
                            ":synopsis": anime.synopsis,
                            ":title": anime.title,
                            ":rating": anime.rating,
                            ":poster_large": anime.poster_large,
                            ":last_update": anime.last_update,
                        })?;
                    }
                }
                transaction.commit()?;

                Result::<_, anyhow::Error>::Ok(anime)
            })
            .await??;
        Ok(())
    }

    /// Upsert kitsu episodes
    pub async fn upsert_kitsu_episodes<E>(&self, episodes: E) -> anyhow::Result<()>
    where
        E: AsSlice<KitsuAnimeEpisode> + Send + 'static,
    {
        self.database
            .access_db(move |database| {
                let transaction = database.transaction()?;
                {
                    let mut statement = transaction.prepare_cached(UPDATE_KITSU_EPISODE_SQL)?;
                    let episodes = episodes.as_slice();
                    for episode in episodes.iter() {
                        statement.execute(named_params! {
                            ":episode_id": episode.episode_id.get(),
                            ":anime_id": episode.anime_id.get(),
                            ":title": episode.title,
                            ":synopsis": episode.synopsis,
                            ":length_minutes": episode.length_minutes,
                            ":number": episode.number,
                            ":thumbnail_original": episode.thumbnail_original,
                            ":last_update": episode.last_update,
                        })?;
                    }
                }
                transaction.commit()?;

                Result::<_, anyhow::Error>::Ok(episodes)
            })
            .await??;
        Ok(())
    }

    /// Get a kitsu anime.
    pub async fn get_kitsu_anime(
        &self,
        anime_id: NonZeroU64,
    ) -> anyhow::Result<Option<Arc<KitsuAnime>>> {
        let anime = self
            .database
            .access_db(move |database| {
                let mut statement = database.prepare_cached(GET_KITSU_ANIME_SQL)?;

                let anime = statement
                    .query_row(
                        named_params! {
                            ":id": anime_id.get(),
                        },
                        |row| {
                            let last_update: u64 = row.get("last_update")?;

                            /*
                            match SystemTime::UNIX_EPOCH
                                .elapsed()
                                .map(|duration| duration.as_secs())
                            {
                                Ok(secs) => {
                                    if secs.saturating_sub(last_update) > 10 * 60 {

                                    }

                                    duration
                                }
                                Err(err) => {
                                    return Ok(anyhow::Error::from(err));
                                }
                            }
                            */

                            let id = row.get("id")?;
                            let id = match NonZeroU64::new(id).context("`id` is 0") {
                                Ok(id) => id,
                                Err(err) => {
                                    return Ok(Err(err));
                                }
                            };
                            let slug = row.get("slug")?;
                            let synopsis = row.get("synopsis")?;
                            let title = row.get("title")?;
                            let rating = row.get("rating")?;
                            let poster_large = row.get("poster_large")?;

                            Ok(Result::<Arc<KitsuAnime>, anyhow::Error>::Ok(Arc::new(
                                KitsuAnime {
                                    id,
                                    slug,
                                    synopsis,
                                    title,
                                    rating,
                                    poster_large,
                                    last_update,
                                },
                            )))
                        },
                    )
                    .optional()?
                    .transpose()?;

                Result::<_, anyhow::Error>::Ok(anime)
            })
            .await??;

        Ok(anime)
    }

    /// Optimize the database.
    pub async fn optimize(&self) -> anyhow::Result<()> {
        self.database
            .access_db(|database| {
                database.execute("PRAGMA OPTIMIZE;", [])?;
                database.execute("VACUUM;", [])
            })
            .await
            .context("failed to access database")?
            .context("failed to execute shutdown commands")?;

        Ok(())
    }

    /// Shut down the database.
    ///
    /// Should only be called once.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let optmize_result = self.optimize().await.map_err(|error| {
            error!("{error}");
            error
        });

        // If we failed to close,
        // the database has already exited,
        // so it is safe to join.
        let close_result = self
            .database
            .close()
            .await
            .context("failed to send close command");
        let join_result = self
            .database
            .join()
            .await
            .context("failed to join database thread");

        join_result.or(close_result).or(optmize_result)
    }
}
