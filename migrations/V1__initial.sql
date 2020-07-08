CREATE TABLE discord_users (
    discord_id BIGINT PRIMARY KEY NOT NULL,
    osu_name VARCHAR(32) NOT NULL
);

CREATE TYPE genre AS enum (
    'unspecified',
    'videogame',
    'anime',
    'rock',
    'pop',
    'other',
    'novelty',
    'hiphop',
    'electronic',
    'metal',
    'classical',
    'folk',
    'jazz',
    'any'
);

CREATE TYPE language AS enum (
    'any',
    'other',
    'english',
    'japanese',
    'chinese',
    'instrumental',
    'korean',
    'french',
    'german',
    'swedish',
    'spanish',
    'italian',
    'russian',
    'polish',
    'unspecified'
);

CREATE TYPE approval_status AS enum (
    'loved',
    'qualified',
    'approved',
    'ranked',
    'pending',
    'wip',
    'graveyard'
);

CREATE TABLE mapsets (
    beatmapset_id INT PRIMARY KEY NOT NULL,
    artist VARCHAR(255) NOT NULL,
    title VARCHAR(255) NOT NULL,
    creator_id INT NOT NULL,
    creator VARCHAR(32) NOT NULL,
    genre genre NOT NULL,
    language language NOT NULL,
    approval_status approval_status NOT NULL,
    approved_date TIMESTAMP DEFAULT NULL
);

CREATE TYPE mode AS enum (
    'osu',
    'taiko',
    'fruits',
    'mania'
);

CREATE TABLE maps (
    beatmap_id INT NOT NULL,
    beatmapset_id INT NOT NULL,
    mode mode NOT NULL,
    version VARCHAR(255) NOT NULL,
    seconds_drain INT NOT NULL,
    seconds_total INT NOT NULL,
    bpm FLOAT NOT NULL,
    stars FLOAT NOT NULL,
    diff_cs FLOAT NOT NULL,
    diff_od FLOAT NOT NULL,
    diff_ar FLOAT NOT NULL,
    diff_hp FLOAT NOT NULL,
    count_circle INT NOT NULL,
    count_slider INT NOT NULL,
    count_spinner INT NOT NULL,
    max_combo INT,
    PRIMARY KEY (beatmap_id),
    FOREIGN KEY (beatmapset_id) REFERENCES mapsets(beatmapset_id)
);

CREATE TABLE ctb_pp (
    beatmap_id INT NOT NULL,
    values JSON NOT NULL,
    FOREIGN KEY (beatmap_id) REFERENCES maps(beatmap_id),
    PRIMARY KEY (beatmap_id)
);

CREATE TABLE ctb_stars (
    beatmap_id INT NOT NULL,
    values JSON NOT NULL,
    FOREIGN KEY (beatmap_id) REFERENCES maps(beatmap_id),
    PRIMARY KEY (beatmap_id)
);

CREATE TABLE mania_pp (
    beatmap_id INT NOT NULL,
    values JSON NOT NULL,
    FOREIGN KEY (beatmap_id) REFERENCES maps(beatmap_id),
    PRIMARY KEY (beatmap_id)
);

CREATE TABLE mania_stars (
    beatmap_id INT NOT NULL,
    values JSON NOT NULL,
    FOREIGN KEY (beatmap_id) REFERENCES maps(beatmap_id),
    PRIMARY KEY (beatmap_id)
);

CREATE TABLE role_assign (
    channel BIGINT NOT NULL,
    message BIGINT NOT NULL,
    role BIGINT NOT NULL
);

CREATE TABLE twitch_users (
    user_id BIGINT PRIMARY KEY NOT NULL,
    name VARCHAR(31) NOT NULL
);

CREATE TABLE stream_tracks (
    channel_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES twitch_users(user_id)
);

CREATE TABLE bggame_stats (
    discord_id BIGINT PRIMARY KEY NOT NULL,
    score INT NOT NULL
);

CREATE TABLE bg_verified(user_id BIGINT PRIMARY KEY NOT NULL);

CREATE TABLE map_tags(
    beatmapset_id INT PRIMARY KEY NOT NULL,
    filetype VARCHAR(7) NOT NULL,
    mode mode NOT NULL,
    farm BOOLEAN DEFAULT false,
    streams BOOLEAN DEFAULT false,
    alternate BOOLEAN DEFAULT false,
    old BOOLEAN DEFAULT false,
    meme BOOLEAN DEFAULT false,
    hardname BOOLEAN DEFAULT false,
    easy BOOLEAN DEFAULT false,
    hard BOOLEAN DEFAULT false,
    tech BOOLEAN DEFAULT false,
    weeb BOOLEAN DEFAULT false,
    bluesky BOOLEAN DEFAULT false,
    english BOOLEAN DEFAULT false,
    kpop BOOLEAN DEFAULT false
);

CREATE TABLE guilds (
    guild_id BIGINT PRIMARY KEY NOT NULL,
    config JSON NOT NULL
);

CREATE TABLE ratio_table (
    name VARCHAR(31) PRIMARY KEY NOT NULL,
    scores CHAR[] NOT NULL,
    ratios REAL[] NOT NULL,
    misses REAL[] NOT NULL
)
