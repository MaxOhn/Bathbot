DROP TABLE maps;
DROP TABLE mapsets;
DROP TABLE role_assigns;
DROP TABLE stream_tracks;
DROP TABLE bggame_scores;
DROP TABLE map_tags;
DROP TABLE guild_configs;
DROP TABLE osu_trackings;
DROP TABLE snipe_countries;
DROP TABLE user_configs;
DROP TABLE osekai_medals;

DROP TRIGGER update_osu_user_stats_last_update ON osu_user_stats;
DROP TRIGGER update_osu_user_stats_mode_last_update ON osu_user_stats_mode;

DROP FUNCTION set_last_update();

DROP TABLE osu_user_names;
DROP TABLE osu_user_stats;
DROP TABLE osu_user_stats_mode;