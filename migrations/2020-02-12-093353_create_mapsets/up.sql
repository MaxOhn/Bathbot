CREATE TABLE mapsets (
    beatmapset_id INT UNSIGNED NOT NULL,
    artist VARCHAR(128) NOT NULL,
    title VARCHAR(128) NOT NULL,
    creator_id INT UNSIGNED NOT NULL,
    creator VARCHAR(32) NOT NULL,
    genre TINYINT UNSIGNED NOT NULL,
    language TINYINT UNSIGNED NOT NULL,
    approval_status TINYINT NOT NULL,
    approved_date TIMESTAMP NULL DEFAULT NULL,
    PRIMARY KEY (beatmapset_id)
)