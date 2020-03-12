CREATE TABLE stars_mania_mods (
    beatmap_id INT UNSIGNED NOT NULL,
    DT FLOAT,
    HT FLOAT,
    FOREIGN KEY (beatmap_id) REFERENCES maps(beatmap_id),
    PRIMARY KEY (beatmap_id)
)