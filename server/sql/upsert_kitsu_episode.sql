INSERT OR REPLACE INTO kitsu_episodes (
    episode_id,
    anime_id,
    title,
    synopsis,
    length_minutes,
    number,
    thumbnail_original,
    last_update
) VALUES (
   :episode_id,
   :anime_id,
   :title,
   :synopsis,
   :length_minutes,
   :number,
   :thumbnail_original,
   :last_update
);