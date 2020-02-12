table! {
    beatmaps (beatmap_id) {
        beatmap_id -> Unsigned<Integer>,
        beatmapset_id -> Unsigned<Integer>,
        mode -> Unsigned<Tinyint>,
        artist -> Varchar,
        title -> Varchar,
        version -> Varchar,
        creator_id -> Unsigned<Integer>,
        creator -> Varchar,
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
        genre -> Unsigned<Tinyint>,
        language -> Unsigned<Tinyint>,
        approval_status -> Tinyint,
        approved_date -> Nullable<Timestamp>,
    }
}

table! {
    beatmaps_pp (beatmap_id) {
        beatmap_id -> Unsigned<Integer>,
        NM -> Float,
        HD -> Float,
        HR -> Float,
        DT -> Float,
        EZ -> Float,
        NF -> Float,
        HDHR -> Float,
        HDDT -> Float,
    }
}

table! {
    beatmaps_stars (beatmap_id) {
        beatmap_id -> Unsigned<Integer>,
        HR -> Float,
        DT -> Float,
    }
}

table! {
    discord_users (discord_id) {
        discord_id -> Unsigned<Bigint>,
        osu_name -> Varchar,
    }
}

allow_tables_to_appear_in_same_query!(beatmaps, beatmaps_pp, beatmaps_stars, discord_users,);
