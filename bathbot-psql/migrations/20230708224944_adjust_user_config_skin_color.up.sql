ALTER TABLE user_render_settings ADD COLUMN use_skin_colors BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE user_render_settings ALTER COLUMN use_skin_colors DROP DEFAULT;
UPDATE user_render_settings SET use_skin_colors = NOT use_beatmap_colors;

ALTER TABLE guild_configs ADD COLUMN hide_medal_solution INT2;

ALTER TABLE guild_configs RENAME COLUMN show_retries TO retries;
ALTER TABLE guild_configs ALTER COLUMN retries TYPE INT2 USING CASE WHEN retries = true THEN 1 ELSE 0 END;

ALTER TABLE user_configs RENAME COLUMN show_retries TO retries;
ALTER TABLE user_configs ALTER COLUMN retries TYPE INT2 USING CASE WHEN retries = true THEN 1 ELSE 0 END;
