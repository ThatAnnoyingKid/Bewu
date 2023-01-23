PRAGMA page_size = 4096;
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS anime (
    id INTEGER NOT NULL UNIQUE PRIMARY KEY,
    
    -- 0: Not Installed | The anime is considered not downloaded, though metadata may still exist.
    -- 1: Installed     | The anime is considered downloaded, and metadata will exist. Thumbnails/covers must exist.
    status INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS kitsu_anime (
    id INTEGER NOT NULL UNIQUE PRIMARY KEY,
    
    slug TEXT NOT NULL UNIQUE,
    synopsis TEXT,
    title TEXT NOT NULL,
    rating TEXT,
    
    poster_large TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS kitsu_episodes (
    episode_id INTEGER NOT NULL UNIQUE PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    
    title TEXT,
    synopsis TEXT,
    length_minutes INTEGER NOT NULL,
    
    thumbnail_original TEXT,
    
    FOREIGN KEY (anime_id) REFERENCES kitsu_anime (id)
) STRICT;