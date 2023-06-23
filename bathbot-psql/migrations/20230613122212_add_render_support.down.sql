DROP TABLE osu_replays;
DROP TABLE user_render_settings;
ALTER TABLE user_configs DROP COLUMN render_button;
ALTER TABLE guild_configs DROP COLUMN render_button, DROP COLUMN allow_custom_skins;
DROP TABLE render_video_urls;