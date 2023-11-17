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