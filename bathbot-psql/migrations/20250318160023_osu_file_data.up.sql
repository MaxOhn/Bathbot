DROP TABLE osu_map_files;

CREATE TABLE IF NOT EXISTS osu_map_file_content (
    map_id  INT4 NOT NULL,
    content BYTEA NOT NULL,
    PRIMARY KEY (map_id)
);