CREATE TABLE IF NOT EXISTS osu_scores_performance (
    score_id INT8 NOT NULL,
    pp       FLOAT8,
    PRIMARY KEY (score_id)
);

CREATE TABLE IF NOT EXISTS osu_scores (
    score_id  INT8 NOT NULL,
    user_id   INT4 NOT NULL,
    map_id    INT4 NOT NULL,
    gamemode  INT2 NOT NULL,
    mods      INT4 NOT NULL,
    score     INT4 NOT NULL,
    maxcombo  INT4 NOT NULL,
    grade     INT2 NOT NULL,
    count50   INT4 NOT NULL,
    count100  INT4 NOT NULL,
    count300  INT4 NOT NULL,
    countmiss INT4 NOT NULL,
    countgeki INT4 NOT NULL,
    countkatu INT4 NOT NULL,
    perfect   BOOL NOT NULL,
    ended_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (score_id)
);

CREATE MATERIALIZED VIEW user_scores AS
SELECT
    osu_scores.*,
    osu_user_stats.country_code,
    osu_scores_performance.pp
FROM
    osu_scores
        JOIN osu_user_stats USING (user_id)
        JOIN osu_scores_performance USING (score_id);

CREATE UNIQUE INDEX user_scores_score_id_index ON user_scores (score_id);
CREATE INDEX user_scores_mode_pp_index ON user_scores (gamemode, pp DESC);
CREATE INDEX user_scores_mode_country_user_index ON user_scores (gamemode, country_code, user_id);
CREATE INDEX user_scores_mode_country_pp_index ON user_scores (gamemode, country_code, pp DESC);
