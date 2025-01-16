DROP INDEX tracked_osu_users_channel_index;

DROP TABLE tracked_osu_users;
DROP TABLE osu_users_100th_pp;

CREATE TABLE IF NOT EXISTS tracked_osu_users (
    user_id     INT4 NOT NULL,
    gamemode    INT2 NOT NULL,
    channels    BYTEA NOT NULL,
    last_update TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, gamemode)
);

ALTER TABLE guild_configs ADD COLUMN osu_track_limit INT2;