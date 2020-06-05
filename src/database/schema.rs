table! {
    bggame_stats (discord_id) {
        discord_id -> Unsigned<Bigint>,
        score -> Unsigned<Integer>,
    }
}

table! {
    discord_users (discord_id) {
        discord_id -> Unsigned<Bigint>,
        osu_name -> Varchar,
    }
}

table! {
    guilds (guild_id) {
        guild_id -> Unsigned<Bigint>,
        with_lyrics -> Bool,
        authorities -> Varchar,
    }
}

table! {
    maps (beatmap_id) {
        beatmap_id -> Unsigned<Integer>,
        beatmapset_id -> Unsigned<Integer>,
        mode -> Unsigned<Tinyint>,
        version -> Varchar,
        seconds_drain -> Unsigned<Integer>,
        seconds_total -> Unsigned<Integer>,
        bpm -> Float,
        stars -> Float,
        diff_cs -> Float,
        diff_od -> Float,
        diff_ar -> Float,
        diff_hp -> Float,
        count_circle -> Unsigned<Integer>,
        count_slider -> Unsigned<Integer>,
        count_spinner -> Unsigned<Integer>,
        max_combo -> Nullable<Unsigned<Integer>>,
    }
}

table! {
    mapsets (beatmapset_id) {
        beatmapset_id -> Unsigned<Integer>,
        artist -> Varchar,
        title -> Varchar,
        creator_id -> Unsigned<Integer>,
        creator -> Varchar,
        genre -> Unsigned<Tinyint>,
        language -> Unsigned<Tinyint>,
        approval_status -> Tinyint,
        approved_date -> Nullable<Timestamp>,
    }
}

table! {
    pp_ctb_mods (beatmap_id) {
        beatmap_id -> Unsigned<Integer>,
        NM -> Nullable<Float>,
        HD -> Nullable<Float>,
        HR -> Nullable<Float>,
        DT -> Nullable<Float>,
        HDHR -> Nullable<Float>,
        HDDT -> Nullable<Float>,
    }
}

table! {
    pp_mania_mods (beatmap_id) {
        beatmap_id -> Unsigned<Integer>,
        NM -> Nullable<Float>,
        NF -> Nullable<Float>,
        EZ -> Nullable<Float>,
        DT -> Nullable<Float>,
        HT -> Nullable<Float>,
        NFEZ -> Nullable<Float>,
        NFDT -> Nullable<Float>,
        EZDT -> Nullable<Float>,
        NFHT -> Nullable<Float>,
        EZHT -> Nullable<Float>,
        NFEZDT -> Nullable<Float>,
        NFEZHT -> Nullable<Float>,
    }
}

table! {
    ratio_table (name) {
        name -> Varchar,
        scores -> Varchar,
        ratios -> Varchar,
        misses -> Varchar,
    }
}

table! {
    role_assign (id) {
        id -> Unsigned<Integer>,
        channel -> Unsigned<Bigint>,
        message -> Unsigned<Bigint>,
        role -> Unsigned<Bigint>,
    }
}

table! {
    stars_ctb_mods (beatmap_id) {
        beatmap_id -> Unsigned<Integer>,
        EZ -> Nullable<Float>,
        HR -> Nullable<Float>,
        DT -> Nullable<Float>,
        HT -> Nullable<Float>,
        EZDT -> Nullable<Float>,
        HRDT -> Nullable<Float>,
        EZHT -> Nullable<Float>,
        HRHT -> Nullable<Float>,
    }
}

table! {
    stars_mania_mods (beatmap_id) {
        beatmap_id -> Unsigned<Integer>,
        DT -> Nullable<Float>,
        HT -> Nullable<Float>,
    }
}

table! {
    stream_tracks (id) {
        id -> Unsigned<Integer>,
        channel_id -> Unsigned<Bigint>,
        user_id -> Unsigned<Bigint>,
        platform -> Unsigned<Tinyint>,
    }
}

table! {
    twitch_users (user_id) {
        user_id -> Unsigned<Bigint>,
        name -> Varchar,
    }
}

joinable!(maps -> mapsets (beatmapset_id));
joinable!(pp_ctb_mods -> maps (beatmap_id));
joinable!(pp_mania_mods -> maps (beatmap_id));
joinable!(stars_ctb_mods -> maps (beatmap_id));
joinable!(stars_mania_mods -> maps (beatmap_id));
joinable!(stream_tracks -> twitch_users (user_id));

allow_tables_to_appear_in_same_query!(
    bggame_stats,
    discord_users,
    guilds,
    maps,
    mapsets,
    pp_ctb_mods,
    pp_mania_mods,
    ratio_table,
    role_assign,
    stars_ctb_mods,
    stars_mania_mods,
    stream_tracks,
    twitch_users,
);
