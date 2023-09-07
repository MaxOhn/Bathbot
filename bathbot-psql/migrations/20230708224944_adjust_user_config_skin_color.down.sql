ALTER TABLE user_render_settings DROP COLUMN use_skin_colors;

ALTER TABLE guild_configs DROP COLUMN hide_medal_solution;

ALTER TABLE guild_configs ALTER COLUMN retries TYPE BOOLEAN USING CASE WHEN retries = 0 THEN false ELSE true END;
ALTER TABLE guild_configs RENAME COLUMN retries TO show_retries;

ALTER TABLE user_configs ALTER COLUMN retries TYPE BOOLEAN USING CASE WHEN retries = 0 THEN false ELSE true END;
ALTER TABLE user_configs RENAME COLUMN retries TO show_retries;
