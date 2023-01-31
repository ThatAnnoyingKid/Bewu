INSERT OR REPLACE INTO kitsu_anime (
    id, 
    slug, 
    synopsis, 
    title, 
    rating,
    poster_large,
    last_update
) VALUES (
    :id, 
    :slug, 
    :synopsis,
    :title,
    :rating,
    :poster_large,
    :last_update
);