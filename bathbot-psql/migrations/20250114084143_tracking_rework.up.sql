-- data is migrated via script beforehand
DROP TABLE tracked_osu_users;

CREATE TABLE IF NOT EXISTS tracked_osu_users (
    user_id           INT4 NOT NULL,
    gamemode          INT2 NOT NULL,
    channel_id        INT8 NOT NULL,
    min_index         INT2,
    max_index         INT2,
    min_pp            FLOAT4,
    max_pp            FLOAT4,
    min_combo_percent FLOAT4,
    max_combo_percent FLOAT4,
    PRIMARY KEY (user_id, gamemode, channel_id)
);

CREATE INDEX tracked_osu_users_channel_index ON tracked_osu_users (channel_id);

CREATE TABLE IF NOT EXISTS osu_users_100th_pp (
    user_id  INT4 NOT NULL,
    gamemode INT2 NOT NULL,
    pp       FLOAT4 NOT NULL,
    PRIMARY KEY (user_id, gamemode)
);

ALTER TABLE guild_configs DROP COLUMN osu_track_limit;