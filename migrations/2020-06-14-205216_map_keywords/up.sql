CREATE TABLE map_tags(
    beatmapset_id INT UNSIGNED NOT NULL,
    filetype VARCHAR(8) NOT NULL,
    farm BOOLEAN DEFAULT false,
    streams BOOLEAN DEFAULT false,
    alternate BOOLEAN DEFAULT false,
    old BOOLEAN DEFAULT false,
    meme BOOLEAN DEFAULT false,
    hardname BOOLEAN DEFAULT false,
    easy BOOLEAN DEFAULT false,
    hard BOOLEAN DEFAULT false,
    tech BOOLEAN DEFAULT false,
    weeb BOOLEAN DEFAULT false,
    bluesky BOOLEAN DEFAULT false,
    english BOOLEAN DEFAULT false,
    PRIMARY KEY (beatmapset_id)
)