CREATE TABLE higherlower_scores (
    discord_id INT8 NOT NULL,
    version    INT2 NOT NULL,
    highscore  INT4 NOT NULL,

    PRIMARY KEY (discord_id, version)
);