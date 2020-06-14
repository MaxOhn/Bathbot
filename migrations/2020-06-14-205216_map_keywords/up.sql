CREATE TABLE map_tags(
    beatmapset_id INT UNSIGNED NOT NULL,
    farm BOOLEAN DEFAULT true,
    streams BOOLEAN DEFAULT true,
    alternate BOOLEAN DEFAULT true,
    old BOOLEAN DEFAULT true,
    meme BOOLEAN DEFAULT true,
    hardname BOOLEAN DEFAULT true,
    easy BOOLEAN DEFAULT true,
    hard BOOLEAN DEFAULT true,
    tech BOOLEAN DEFAULT true,
    weeb BOOLEAN DEFAULT true,
    bluesky BOOLEAN DEFAULT true,
    english BOOLEAN DEFAULT true,
    FOREIGN KEY (beatmapset_id) REFERENCES mapsets(beatmapset_id),
    PRIMARY KEY (beatmapset_id)
)