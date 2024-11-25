ALTER TABLE osu_map_difficulty DROP COLUMN aim_difficult_strain_count;
ALTER TABLE osu_map_difficulty DROP COLUMN speed_difficult_strain_count;
ALTER TABLE osu_map_difficulty DROP COLUMN n_large_ticks;

ALTER TABLE osu_map_difficulty_taiko RENAME COLUMN great_hit_window TO hit_window;
ALTER TABLE osu_map_difficulty_taiko DROP COLUMN ok_hit_window;
ALTER TABLE osu_map_difficulty_taiko DROP COLUMN mono_stamina_factor;

ALTER TABLE osu_map_difficulty_mania DROP COLUMN n_hold_notes;

DELETE FROM guild_configs;
ALTER TABLE guild_configs ALTER COLUMN prefixes TYPE BYTEA USING (''::BYTEA);