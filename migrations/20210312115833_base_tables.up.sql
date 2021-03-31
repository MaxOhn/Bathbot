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

CREATE TABLE discord_user_links (
    discord_id INT8 NOT NULL,
    osu_name   VARCHAR(16) NOT NULL,

    PRIMARY KEY (discord_id)
);

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
    config   JSON NOT NULL,

    PRIMARY KEY (guild_id)
);

CREATE TABLE osu_trackings (
    user_id        INT4 NOT NULL,
    mode           INT2 NOT NULL,
    last_top_score TIMESTAMPTZ NOT NULL,
    channels       JSON NOT NULL,

    PRIMARY KEY (user_id, mode)
);
