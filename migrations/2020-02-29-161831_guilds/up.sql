CREATE TABLE guilds (
    guild_id BIGINT UNSIGNED PRIMARY KEY,
    with_lyrics BOOLEAN NOT NULL,
    authorities VARCHAR(256) NOT NULL
)