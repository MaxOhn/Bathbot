-- scary stuff
DELETE FROM osu_map_difficulty_taiko;
DELETE FROM osu_map_difficulty_catch;
DELETE FROM osu_map_difficulty_mania;

ALTER TABLE osu_map_difficulty_taiko ADD COLUMN is_convert BOOLEAN NOT NULL;
ALTER TABLE osu_map_difficulty_taiko RENAME COLUMN colour TO color;

ALTER TABLE osu_map_difficulty_catch ADD COLUMN is_convert BOOLEAN NOT NULL;

ALTER TABLE osu_map_difficulty_mania ADD COLUMN n_objects INT4 NOT NULL;
ALTER TABLE osu_map_difficulty_mania ADD COLUMN is_convert BOOLEAN NOT NULL;