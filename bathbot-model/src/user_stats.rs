use time::OffsetDateTime;
use twilight_interactions::command::{CommandOption, CreateOption};

pub enum UserStatsEntries {
    Accuracy(Vec<UserStatsEntry<f32>>),
    Amount(Vec<UserStatsEntry<u64>>),
    AmountWithNegative(Vec<UserStatsEntry<i64>>),
    Date(Vec<UserStatsEntry<OffsetDateTime>>),
    Float(Vec<UserStatsEntry<f32>>),
    Playtime(Vec<UserStatsEntry<u32>>),
    PpF32(Vec<UserStatsEntry<f32>>),
    Rank(Vec<UserStatsEntry<u32>>),
}

// Be sure to keep structure in sync with
// `bathbot_psql::model::osu::user::DbUserStatsEntry`!
pub struct UserStatsEntry<V> {
    pub country: [u8; 2],
    pub name: String,
    pub value: V,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum UserStatsColumn {
    #[option(name = "Badges", value = "badges")]
    Badges,
    #[option(name = "Comments", value = "comments")]
    Comments,
    #[option(name = "Followers", value = "followers")]
    Followers,
    #[option(name = "Forum posts", value = "forum_posts")]
    ForumPosts,
    #[option(name = "Graveyard mapsets", value = "graveyard_mapsets")]
    GraveyardMapsets,
    #[option(name = "Join date", value = "join_date")]
    JoinDate,
    #[option(name = "Kudosu Available", value = "kudosu_available")]
    KudosuAvailable,
    #[option(name = "Kudosu Total", value = "kudosu_total")]
    KudosuTotal,
    #[option(name = "Loved mapsets", value = "loved_mapsets")]
    LovedMapsets,
    #[option(name = "Mapping followers", value = "mapping_followers")]
    Subscribers,
    #[option(name = "Medals", value = "medals")]
    Medals,
    #[option(name = "Namechanges", value = "namechanges")]
    Namechanges,
    #[option(name = "Played maps", value = "played_maps")]
    PlayedMaps,
    #[option(name = "Ranked mapsets", value = "ranked_mapsets")]
    RankedMapsets,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum UserModeStatsColumn {
    #[option(name = "Accuracy", value = "acc")]
    Accuracy,
    #[option(name = "Average hits per play", value = "avg_hits")]
    AverageHits,
    #[option(name = "Count SSH", value = "count_ssh")]
    CountSsh,
    #[option(name = "Count SS", value = "count_ss")]
    CountSs,
    #[option(name = "Total SS", value = "total_ss")]
    TotalSs,
    #[option(name = "Count SH", value = "count_sh")]
    CountSh,
    #[option(name = "Count S", value = "count_s")]
    CountS,
    #[option(name = "Total S", value = "total_s")]
    TotalS,
    #[option(name = "Count A", value = "count_a")]
    CountA,
    #[option(name = "Level", value = "level")]
    Level,
    #[option(name = "Max combo", value = "max_combo")]
    MaxCombo,
    #[option(name = "Playcount", value = "playcount")]
    Playcount,
    #[option(name = "Playtime", value = "playtime")]
    Playtime,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "PP per Month", value = "pp_per_month")]
    PpPerMonth,
    #[option(name = "Country rank", value = "country_rank")]
    RankCountry,
    #[option(name = "Global rank", value = "global_rank")]
    RankGlobal,
    #[option(name = "Replays watched", value = "replays")]
    ReplaysWatched,
    #[option(name = "Ranked score", value = "ranked_score")]
    ScoreRanked,
    #[option(name = "Total score", value = "total_score")]
    ScoreTotal,
    #[option(name = "Global number 1s", value = "global_firsts")]
    ScoresFirst,
    #[option(name = "Top PP", value = "top1")]
    Top1,
    #[option(name = "Total hits", value = "total_hits")]
    TotalHits,
    #[option(name = "Top PP range", value = "top_range")]
    TopRange,
}
