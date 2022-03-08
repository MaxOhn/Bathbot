ALTER TABLE maps ADD COLUMN user_id INT4 NOT NULL DEFAULT 0;

ALTER TABLE guild_configs DROP COLUMN track_limit;

ALTER TABLE user_configs RENAME COlUMN embeds_size TO embeds_maximized;
ALTER TABLE user_configs ALTER COLUMN embeds_maximized TYPE BOOL USING CASE WHEN embeds_maximized = 0 THEN FALSE ELSE TRUE END;

ALTER TABLE guild_configs RENAME COlUMN embeds_size TO embeds_maximized;
ALTER TABLE guild_configs ALTER COLUMN embeds_maximized TYPE BOOL USING CASE WHEN embeds_maximized = 0 THEN FALSE ELSE TRUE END;