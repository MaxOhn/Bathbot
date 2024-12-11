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

CREATE TABLE IF NOT EXISTS osu_map_difficulty (
    map_id                       INT4 NOT NULL,
    mods                         INT4 NOT NULL,
    aim                          FLOAT8 NOT NULL,
    speed                        FLOAT8 NOT NULL,
    flashlight                   FLOAT8 NOT NULL,
    slider_factor                FLOAT8 NOT NULL,
    speed_note_count             FLOAT8 NOT NULL,
    ar                           FLOAT8 NOT NULL,
    od                           FLOAT8 NOT NULL,
    hp                           FLOAT8 NOT NULL,
    n_circles                    INT4 NOT NULL,
    n_sliders                    INT4 NOT NULL,
    n_spinners                   INT4 NOT NULL,
    stars                        FLOAT8 NOT NULL,
    max_combo                    INT4 NOT NULL,
    aim_difficult_strain_count   FLOAT8 NOT NULL,
    speed_difficult_strain_count FLOAT8 NOT NULL,
    n_large_ticks                INT4 NOT NULL,
    PRIMARY KEY (map_id, mods)
);

CREATE TABLE IF NOT EXISTS osu_map_difficulty_catch (
    map_id          INT4 NOT NULL,
    mods            INT4 NOT NULL,
    stars           FLOAT8 NOT NULL,
    ar              FLOAT8 NOT NULL,
    n_fruits        INT4 NOT NULL,
    n_droplets      INT4 NOT NULL,
    n_tiny_droplets INT4 NOT NULL,
    is_convert      BOOL NOT NULL,
    PRIMARY KEY (map_id, mods)
);

CREATE TABLE IF NOT EXISTS osu_map_difficulty_mania (
    map_id       INT4 NOT NULL,
    mods         INT4 NOT NULL,
    stars        FLOAT8 NOT NULL,
    hit_window   FLOAT8 NOT NULL,
    max_combo    INT4 NOT NULL,
    n_objects    INT4 NOT NULL,
    is_convert   BOOL NOT NULL,
    n_hold_notes INT4 NOT NULL,
    PRIMARY KEY (map_id, mods)
);

CREATE TABLE IF NOT EXISTS osu_map_difficulty_taiko (
    map_id              INT4 NOT NULL,
    mods                INT4 NOT NULL,
    stamina             FLOAT8 NOT NULL,
    rhythm              FLOAT8 NOT NULL,
    color               FLOAT8 NOT NULL,
    peak                FLOAT8 NOT NULL,
    great_hit_window    FLOAT8 NOT NULL,
    stars               FLOAT8 NOT NULL,
    max_combo           INT4 NOT NULL,
    is_convert          BOOL NOT NULL,
    ok_hit_window       FLOAT8 NOT NULL,
    mono_stamina_factor FLOAT8 NOT NULL,
    PRIMARY KEY (map_id, mods)
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
