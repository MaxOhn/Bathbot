CREATE TABLE maps (
    map_id         INT4 NOT NULL,
    mapset_id      INT4 NOT NULL,
    user_id        INT4 NOT NULL DEFAULT 0,
    checksum       VARCHAR(32),
    version        VARCHAR(80) NOT NULL DEFAULT '',
    seconds_total  INT4 NOT NULL DEFAULT 0,
    seconds_drain  INT4 NOT NULL DEFAULT 0,
    count_circles  INT4 NOT NULL DEFAULT 0,
    count_sliders  INT4 NOT NULL DEFAULT 0,
    count_spinners INT4 NOT NULL DEFAULT 0,
    hp             FLOAT4 NOT NULL DEFAULT 0,
    cs             FLOAT4 NOT NULL DEFAULT 0,
    od             FLOAT4 NOT NULL DEFAULT 0,
    ar             FLOAT4 NOT NULL DEFAULT 0,
    mode           INT2 NOT NULL DEFAULT 0,
    status         INT2 NOT NULL DEFAULT 0,
    last_update    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    stars          FLOAT4 NOT NULL DEFAULT 0,
    bpm            FLOAT4 NOT NULL DEFAULT 0,
    max_combo      INT4,

    PRIMARY KEY (map_id)
);

CREATE INDEX maps_mapset_id ON maps (mapset_id);
CREATE INDEX maps_user_id ON maps (user_id);

CREATE TABLE mapsets (
    mapset_id   INT4 NOT NULL,
    user_id     INT4 NOT NULL DEFAULT 0,
    artist      VARCHAR(80) NOT NULL DEFAULT '',
    title       VARCHAR(80) NOT NULL DEFAULT '',
    creator     VARCHAR(80) NOT NULL DEFAULT '',
    bpm         FLOAT4 NOT NULL DEFAULT 0,
    status      INT2 NOT NULL DEFAULT 0,
    ranked_date TIMESTAMPTZ NOT NULL,
    genre       INT2 NOT NULL DEFAULT 1,
    language    INT2 NOT NULL DEFAULT 1,

    PRIMARY KEY (mapset_id)
);

CREATE INDEX mapsets_user_id ON mapsets (user_id);
CREATE INDEX mapsets_status ON mapsets (status);

CREATE TABLE role_assigns (
    channel_id INT8 NOT NULL,
    message_id INT8 NOT NULL,
    role_id    INT8 NOT NULL,

    PRIMARY KEY (channel_id, message_id, role_id)
);

CREATE TABLE stream_tracks (
    channel_id INT8 NOT NULL,
    user_id    INT8 NOT NULL,

    PRIMARY KEY (channel_id, user_id)
);

CREATE TABLE bggame_scores (
    discord_id INT8 NOT NULL,
    score      INT4 NOT NULL DEFAULT 0,

    PRIMARY KEY (discord_id)
);

CREATE TABLE map_tags (
    mapset_id INT4 NOT NULL,
    filename  VARCHAR(16) NOT NULL,
    mode      INT2 NOT NULL,
    
    farm      BOOL NOT NULL DEFAULT FALSE,
    streams   BOOL NOT NULL DEFAULT FALSE,
    alternate BOOL NOT NULL DEFAULT FALSE,
    old       BOOL NOT NULL DEFAULT FALSE,
    meme      BOOL NOT NULL DEFAULT FALSE,
    hardname  BOOL NOT NULL DEFAULT FALSE,
    easy      BOOL NOT NULL DEFAULT FALSE,
    hard      BOOL NOT NULL DEFAULT FALSE,
    tech      BOOL NOT NULL DEFAULT FALSE,
    weeb      BOOL NOT NULL DEFAULT FALSE,
    bluesky   BOOL NOT NULL DEFAULT FALSE,
    english   BOOL NOT NULL DEFAULT FALSE,
    kpop      BOOL NOT NULL DEFAULT FALSE,

    PRIMARY KEY (mapset_id)
);

CREATE TABLE guild_configs (
    guild_id INT8 NOT NULL,
    authorities   BYTEA NOT NULL,
    prefixes      BYTEA NOT NULL,
    with_lyrics   BOOL,

    PRIMARY KEY (guild_id)
);

CREATE TABLE osu_trackings (
    user_id        INT4 NOT NULL,
    mode           INT2 NOT NULL,
    last_top_score TIMESTAMPTZ NOT NULL,
    channels       JSON NOT NULL,

    PRIMARY KEY (user_id, mode)
);

CREATE TABLE snipe_countries (
    name VARCHAR(32) NOT NULL,
    code VARCHAR(2) NOT NULL,

    PRIMARY KEY (name)
);

CREATE TABLE user_configs (
    discord_id       INT8 NOT NULL,
    osu_id           INT4,
    mode             INT2,
    profile_size     INT2,
    twitch_id        INT8,
    embeds_maximized BOOL,
    show_retries     BOOL,

    PRIMARY KEY (discord_id)
);

CREATE TABLE osekai_medals (
    medal_id    INT4 NOT NULL,
    name        TEXT NOT NULL,
    icon_url    TEXT NOT NULL,
    description TEXT NOT NULL,
    restriction INT2,
    grouping    TEXT NOT NULL,
    solution    TEXT,
    mods        INT4,
    mode_order  INT8 NOT NULL,
    ordering    INT8 NOT NULL,

    PRIMARY KEY (medal_id)
);

CREATE INDEX osekai_medal_name ON osekai_medals (name);
CREATE INDEX osekai_medal_grouping ON osekai_medals (grouping);

CREATE TABLE osu_user_names (
    user_id  INT4 NOT NULL,
    username VARCHAR(15) NOT NULL,
    
    PRIMARY KEY (user_id)
);

CREATE INDEX osu_user_name ON osu_user_names (username);

CREATE TABLE osu_user_stats (
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
    last_update              TIMESTAMPTZ DEFAULT now() NOT NULL,
    
    PRIMARY KEY (user_id)
);

CREATE TABLE osu_user_stats_mode (
    user_id         INT4 NOT NULL,
    mode            INT2 NOT NULL,
    pp              FLOAT4 NOT NULL,
    accuracy        FLOAT4 NOT NULL,
    country_rank    INT4 NOT NULL,
    global_rank     INT4 NOT NULL,
    count_ss        INT4 NOT NULL,
    count_ssh       INT4 NOT NULL,
    count_s         INT4 NOT NULL,
    count_sh        INT4 NOT NULL,
    count_a         INT4 NOT NULL,
    level           FLOAT4 NOT NULL,
    max_combo       INT4 NOT NULL,
    playcount       INT4 NOT NULL,
    playtime        INT4 NOT NULL,
    ranked_score    INT8 NOT NULL,
    replays_watched INT4 NOT NULL,
    total_hits      INT8 NOT NULL,
    total_score     INT8 NOT NULL,
    scores_first    INT4 NOT NULL,
    last_update     TIMESTAMPTZ DEFAULT now() NOT NULL,
    
    PRIMARY KEY (user_id, mode)
);

CREATE FUNCTION set_last_update() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.last_update = now();
    RETURN NEW; 
END;
$$;

CREATE TRIGGER update_osu_user_stats_last_update BEFORE UPDATE ON osu_user_stats FOR EACH ROW EXECUTE PROCEDURE set_last_update();
CREATE TRIGGER update_osu_user_stats_mode_last_update BEFORE UPDATE ON osu_user_stats_mode FOR EACH ROW EXECUTE PROCEDURE set_last_update();