table! {
    discord_users (discord_id) {
        discord_id -> Unsigned<Bigint>,
        osu_name -> Varchar,
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

joinable!(maps -> mapsets (beatmapset_id));

allow_tables_to_appear_in_same_query!(
    discord_users,
    maps,
    mapsets,
);
