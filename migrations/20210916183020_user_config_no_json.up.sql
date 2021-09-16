CREATE TABLE user_config (
    discord_id INT8 NOT NULL,
    osu_username VARCHAR(15),
    mode INT2,
    profile_size INT2,
    twitch_id INT8,
    embeds_maximized BOOL NOT NULL DEFAULT TRUE,
    show_retries BOOL NOT NULL DEFAULT TRUE,

    PRIMARY KEY (discord_id)
);

CREATE INDEX osu ON user_config (osu_username);