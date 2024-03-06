ALTER TABLE osu_map_difficulty_taiko DROP COLUMN is_convert;
ALTER TABLE osu_map_difficulty_taiko RENAME COLUMN color TO colour;
ALTER TABLE osu_map_difficulty_catch DROP COLUMN is_convert;
ALTER TABLE osu_map_difficulty_mania DROP COLUMN n_objects;
ALTER TABLE osu_map_difficulty_mania DROP COLUMN is_convert;