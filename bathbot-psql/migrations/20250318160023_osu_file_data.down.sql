DROP TABLE osu_map_file_content;

CREATE TABLE IF NOT EXISTS osu_map_files (
    map_id       INT4 NOT NULL,
    map_filepath VARCHAR(150) NOT NULL,
    PRIMARY KEY (map_id)
);