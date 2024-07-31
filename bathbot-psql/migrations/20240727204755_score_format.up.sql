ALTER TABLE user_configs DROP COLUMN score_size;
ALTER TABLE user_configs DROP COLUMN minimized_pp;
ALTER TABLE user_configs ADD COLUMN score_embed JSONB;

ALTER TABLE guild_configs DROP COLUMN score_size;
ALTER TABLE guild_configs DROP COLUMN minimized_pp;