CREATE TABLE IF NOT EXISTS osu_maps (
    map_id         INT4 NOT NULL,
    mapset_id      INT4 NOT NULL,
    user_id        INT4 NOT NULL,
    checksum       VARCHAR(32) NOT NULL,
    map_version    VARCHAR(80) NOT NULL,
    seconds_total  INT4 NOT NULL,
    seconds_drain  INT4 NOT NULL,
    count_circles  INT4 NOT NULL,
    count_sliders  INT4 NOT NULL,
    count_spinners INT4 NOT NULL,
    hp             FLOAT4 NOT NULL,
    cs             FLOAT4 NOT NULL,
    od             FLOAT4 NOT NULL,
    ar             FLOAT4 NOT NULL,
    bpm            FLOAT4 NOT NULL,
    gamemode       INT2 NOT NULL,
    max_combo      INT4,
    last_update    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (map_id)
);

CREATE TABLE IF NOT EXISTS osu_map_files (
    map_id       INT4 NOT NULL,
    map_filepath VARCHAR(150) NOT NULL,
    PRIMARY KEY (map_id)
);

CREATE TABLE IF NOT EXISTS osu_map_difficulty (
    map_id           INT4 NOT NULL,
    mods             INT4 NOT NULL,
    aim              FLOAT8 NOT NULL,
    speed            FLOAT8 NOT NULL,
    flashlight       FLOAT8 NOT NULL,
    slider_factor    FLOAT8 NOT NULL,
    speed_note_count FLOAT8 NOT NULL,
    ar               FLOAT8 NOT NULL,
    od               FLOAT8 NOT NULL,
    hp               FLOAT8 NOT NULL,
    n_circles        INT4 NOT NULL,
    n_sliders        INT4 NOT NULL,
    n_spinners       INT4 NOT NULL,
    stars            FLOAT8 NOT NULL,
    max_combo        INT4 NOT NULL,
    PRIMARY KEY (map_id, mods)
);

CREATE TABLE IF NOT EXISTS osu_map_difficulty_taiko (
    map_id     INT4 NOT NULL,
    mods       INT4 NOT NULL,
    stamina    FLOAT8 NOT NULL,
    rhythm     FLOAT8 NOT NULL,
    colour     FLOAT8 NOT NULL,
    peak       FLOAT8 NOT NULL,
    hit_window FLOAT8 NOT NULL,
    stars      FLOAT8 NOT NULL,
    max_combo  INT4 NOT NULL,
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
    PRIMARY KEY (map_id, mods)
);

CREATE TABLE IF NOT EXISTS osu_map_difficulty_mania (
    map_id     INT4 NOT NULL,
    mods       INT4 NOT NULL,
    stars      FLOAT8 NOT NULL,
    hit_window FLOAT8 NOT NULL,
    max_combo  INT4 NOT NULL,
    PRIMARY KEY (map_id, mods)
);

CREATE TABLE IF NOT EXISTS osu_mapsets (
    mapset_id   INT4 NOT NULL,
    user_id     INT4 NOT NULL,
    artist      VARCHAR(80) NOT NULL,
    title       VARCHAR(80) NOT NULL,
    creator     VARCHAR(80) NOT NULL,
    source      VARCHAR(200) NOT NULL,
    tags        VARCHAR(1000) NOT NULL,
    video       BOOL NOT NULL,
    storyboard  BOOL NOT NULL,
    bpm         FLOAT4 NOT NULL,
    rank_status INT2 NOT NULL,
    ranked_date TIMESTAMPTZ,
    genre_id    INT2 NOT NULL,
    language_id INT2 NOT NULL,
    thumbnail   VARCHAR(80) NOT NULL,
    cover       VARCHAR(80) NOT NULL,
    last_update TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (mapset_id)
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

CREATE TABLE IF NOT EXISTS osu_scores_performance (
    score_id INT8 NOT NULL,
    pp       FLOAT8,
    PRIMARY KEY (score_id)
);

CREATE TABLE IF NOT EXISTS tracked_twitch_streams (
    channel_id INT8 NOT NULL,
    user_id    INT8 NOT NULL,
    PRIMARY KEY (channel_id, user_id)
);

CREATE TABLE IF NOT EXISTS bggame_scores (
    discord_id INT8 NOT NULL,
    score      INT4 NOT NULL DEFAULT 0,
    PRIMARY KEY (discord_id)
);

CREATE TABLE IF NOT EXISTS higherlower_scores (
    discord_id   INT8 NOT NULL,
    game_version INT2 NOT NULL,
    highscore    INT4 NOT NULL,
    PRIMARY KEY (discord_id, game_version)
);

CREATE INDEX higherlower_scores_version_index ON higherlower_scores (game_version);

CREATE TABLE IF NOT EXISTS map_tags (
    mapset_id      INT4 NOT NULL,
    image_filename VARCHAR(16) NOT NULL,
    gamemode       INT2 NOT NULL,
    farm           BOOL NOT NULL DEFAULT FALSE,
    streams        BOOL NOT NULL DEFAULT FALSE,
    alternate      BOOL NOT NULL DEFAULT FALSE,
    OLD            BOOL NOT NULL DEFAULT FALSE,
    meme           BOOL NOT NULL DEFAULT FALSE,
    hardname       BOOL NOT NULL DEFAULT FALSE,
    easy           BOOL NOT NULL DEFAULT FALSE,
    hard           BOOL NOT NULL DEFAULT FALSE,
    tech           BOOL NOT NULL DEFAULT FALSE,
    weeb           BOOL NOT NULL DEFAULT FALSE,
    bluesky        BOOL NOT NULL DEFAULT FALSE,
    english        BOOL NOT NULL DEFAULT FALSE,
    kpop           BOOL NOT NULL DEFAULT FALSE,
    PRIMARY KEY (mapset_id)
);

CREATE TABLE IF NOT EXISTS guild_configs (
    guild_id        INT8 NOT NULL,
    -- (de)serialized through rkyv
    authorities     BYTEA NOT NULL,
    -- (de)serialized through rkyv
    prefixes        BYTEA NOT NULL,
    allow_songs     BOOL,
    score_size     INT2,
    show_retries    BOOL,
    osu_track_limit INT2,
    minimized_pp    INT2,
    list_size       INT2,
    PRIMARY KEY (guild_id)
);

CREATE TABLE IF NOT EXISTS user_configs (
    discord_id       INT8 NOT NULL,
    osu_id           INT4,
    gamemode         INT2,
    twitch_id        INT8,
    score_size       INT2,
    show_retries     BOOL,
    minimized_pp     INT2,
    list_size        INT2,
    timezone_seconds INT4,
    PRIMARY KEY (discord_id)
);

CREATE INDEX user_configs_osu_index ON user_configs (osu_id);

CREATE TABLE IF NOT EXISTS tracked_osu_users (
    user_id     INT4 NOT NULL,
    gamemode    INT2 NOT NULL,
    -- (de)serialized through rkyv
    channels    BYTEA NOT NULL,
    last_update TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, gamemode)
);

CREATE TABLE IF NOT EXISTS huismetbenen_countries (
    country_name VARCHAR(32) NOT NULL,
    country_code VARCHAR(2) NOT NULL,
    CHECK (country_code = UPPER(country_code)),
    PRIMARY KEY (country_code)
);

CREATE TABLE IF NOT EXISTS osu_user_names (
    user_id  INT4 NOT NULL,
    username VARCHAR(32) NOT NULL,
    PRIMARY KEY (user_id)
);

CREATE INDEX osu_user_names_name_index ON osu_user_names (username);

CREATE TABLE IF NOT EXISTS osu_user_stats (
    user_id                  INT4 NOT NULL,
    country_code             VARCHAR(2) NOT NULL,
    join_date                TIMESTAMPTZ NOT NULL,
    comment_count            INT4 NOT NULL,
    kudosu_total             INT4 NOT NULL,
    kudosu_available         INT4 NOT NULL,
    forum_post_count         INT4 NOT NULL,
    badges                   INT4 NOT NULL,
    played_maps              INT4 NOT NULL,
    followers                INT4 NOT NULL,
    graveyard_mapset_count   INT4 NOT NULL,
    loved_mapset_count       INT4 NOT NULL,
    mapping_followers        INT4 NOT NULL,
    previous_usernames_count INT4 NOT NULL,
    ranked_mapset_count      INT4 NOT NULL,
    medals                   INT4 NOT NULL,
    last_update              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id)
);

CREATE TABLE IF NOT EXISTS osu_user_mode_stats (
    user_id         INT4 NOT NULL,
    gamemode        INT2 NOT NULL,
    pp              FLOAT4 NOT NULL,
    accuracy        FLOAT4 NOT NULL,
    country_rank    INT4 NOT NULL,
    global_rank     INT4 NOT NULL,
    count_ss        INT4 NOT NULL,
    count_ssh       INT4 NOT NULL,
    count_s         INT4 NOT NULL,
    count_sh        INT4 NOT NULL,
    count_a         INT4 NOT NULL,
    user_level      FLOAT4 NOT NULL,
    max_combo       INT4 NOT NULL,
    playcount       INT4 NOT NULL,
    playtime        INT4 NOT NULL,
    ranked_score    INT8 NOT NULL,
    replays_watched INT4 NOT NULL,
    total_hits      INT8 NOT NULL,
    total_score     INT8 NOT NULL,
    scores_first    INT4 NOT NULL,
    last_update     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, gamemode)
);

CREATE INDEX osu_user_mode_stats_pp_index ON osu_user_mode_stats (pp);

CREATE INDEX osu_user_mode_stats_global_rank_index ON osu_user_mode_stats (global_rank);

CREATE INDEX osu_user_mode_stats_mode_index ON osu_user_mode_stats (gamemode);