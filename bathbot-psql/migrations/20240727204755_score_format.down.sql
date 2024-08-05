ALTER TABLE user_configs ADD COLUMN score_size INT2;
ALTER TABLE user_configs ADD COLUMN minimized_pp INT2;
ALTER TABLE user_configs DROP COLUMN score_embed;

ALTER TABLE guild_configs ADD COLUMN score_size INT2;
ALTER TABLE guild_configs ADD COLUMN minimized_pp INT2;