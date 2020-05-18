CREATE TABLE pp_ctb_mods (
    beatmap_id INT UNSIGNED NOT NULL,
    NM FLOAT,
    HD FLOAT,
    HR FLOAT,
    DT FLOAT,
    HDHR FLOAT,
    HDDT FLOAT,
    FOREIGN KEY (beatmap_id) REFERENCES maps(beatmap_id),
    PRIMARY KEY (beatmap_id)
)