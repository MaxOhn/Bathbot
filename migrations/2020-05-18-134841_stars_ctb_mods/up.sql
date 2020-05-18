CREATE TABLE stars_ctb_mods (
    beatmap_id INT UNSIGNED NOT NULL,
    EZ FLOAT,
    HR FLOAT,
    DT FLOAT,
    HT FLOAT,
    EZDT FLOAT,
    HRDT FLOAT,
    EZHT FLOAT,
    HRHT FLOAT,
    FOREIGN KEY (beatmap_id) REFERENCES maps(beatmap_id),
    PRIMARY KEY (beatmap_id)
)