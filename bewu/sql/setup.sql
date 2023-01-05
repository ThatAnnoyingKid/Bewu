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
    rating TEXT
) STRICT;