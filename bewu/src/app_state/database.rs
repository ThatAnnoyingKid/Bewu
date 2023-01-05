use anyhow::Context;
use async_rusqlite::rusqlite::named_params;
use std::path::Path;
use std::sync::Arc;
use tracing::error;

const SETUP_SQL: &str = include_str!("../../sql/setup.sql");
const UPDATE_KITSU_ANIME_SQL: &str = "
INSERT OR REPLACE INTO kitsu_anime (
    id, 
    slug, 
    synopsis, 
    title, 
    rating
) VALUES (
    :id, 
    :slug, 
    :synopsis,
    :title,
    :rating
);
";

#[derive(Debug, Clone)]
pub struct Anime {
    pub id: u64,
    pub slug: String,
    pub synopsis: String,
    pub title: String,
    pub rating: Option<String>,
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

    /// Update kitsu anime
    pub async fn update_kitsu_anime(&self, anime: Arc<[Anime]>) -> anyhow::Result<()> {
        self.database
            .access_db(move |database| {
                let transaction = database.transaction()?;
                {
                    let mut statement = transaction.prepare_cached(UPDATE_KITSU_ANIME_SQL)?;
                    for anime in anime.iter() {
                        statement.execute(named_params! {
                            ":id": anime.id,
                            ":slug": anime.slug,
                            ":synopsis": anime.synopsis,
                            ":title": anime.title,
                            ":rating": anime.rating,
                        })?;
                    }
                }
                transaction.commit()?;

                Result::<_, anyhow::Error>::Ok(anime)
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