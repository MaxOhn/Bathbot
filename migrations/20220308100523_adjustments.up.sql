ALTER TABLE maps DROP COLUMN user_id;

ALTER TABLE guild_configs ADD COLUMN track_limit INT2;

ALTER TABLE user_configs ALTER COLUMN embeds_maximized TYPE INT2 USING CASE WHEN embeds_maximized THEN 0 ELSE 1 END;
ALTER TABLE user_configs RENAME COLUMN embeds_maximized TO embeds_size;

ALTER TABLE guild_configs ALTER COLUMN embeds_maximized TYPE INT2 USING CASE WHEN embeds_maximized THEN 0 ELSE 1 END;
ALTER TABLE guild_configs RENAME COLUMN embeds_maximized TO embeds_size;

ALTER TABLE user_configs ADD COLUMN minimized_pp INT2;
ALTER TABLE guild_configs ADD COLUMN minimized_pp INT2;