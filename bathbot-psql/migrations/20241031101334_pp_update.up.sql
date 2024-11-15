DELETE FROM osu_map_difficulty;
ALTER TABLE osu_map_difficulty ADD COLUMN aim_difficult_strain_count FLOAT8 NOT NULL;
ALTER TABLE osu_map_difficulty ADD COLUMN speed_difficult_strain_count FLOAT8 NOT NULL;
ALTER TABLE osu_map_difficulty ADD COLUMN n_slider_ticks INT4 NOT NULL;

DELETE FROM osu_map_difficulty_taiko;
ALTER TABLE osu_map_difficulty_taiko RENAME COLUMN hit_window TO great_hit_window;
ALTER TABLE osu_map_difficulty_taiko ADD COLUMN ok_hit_window FLOAT8 NOT NULL;
ALTER TABLE osu_map_difficulty_taiko ADD COLUMN mono_stamina_factor FLOAT8 NOT NULL;

DELETE FROM osu_map_difficulty_mania;
ALTER TABLE osu_map_difficulty_mania ADD COLUMN n_hold_notes INT4 NOT NULL;

DELETE FROM osu_scores_performance;

DELETE FROM guild_configs;
ALTER TABLE guild_configs ALTER COLUMN prefixes TYPE JSONB USING (to_jsonb(prefixes));