CREATE TABLE pp_mania_mods (
    beatmap_id INT UNSIGNED NOT NULL,
    NM FLOAT,
    NF FLOAT,
    EZ FLOAT,
    DT FLOAT,
    HT FLOAT,
    NFEZ FLOAT,
    NFDT FLOAT,
    EZDT FLOAT,
    NFHT FLOAT,
    EZHT FLOAT,
    NFEZDT FLOAT,
    NFEZHT FLOAT,
    FOREIGN KEY (beatmap_id) REFERENCES maps(beatmap_id),
    PRIMARY KEY (beatmap_id)
)