CREATE TABLE stream_tracks (
    id INT UNSIGNED AUTO_INCREMENT,
    channel_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    platform TINYINT UNSIGNED NOT NULL,
    PRIMARY KEY (id),
    FOREIGN KEY (user_id) REFERENCES twitch_users(user_id)
)