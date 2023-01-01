use sqlx::{
    database::HasValueRef, error::BoxDynError, postgres::PgTypeInfo, Decode, FromRow, Postgres,
    Type,
};
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

struct DbCountryCode {
    inner: [u8; 2],
}

impl<'r> Decode<'r, Postgres> for DbCountryCode {
    #[inline]
    fn decode(value: <Postgres as HasValueRef<'r>>::ValueRef) -> Result<Self, BoxDynError> {
        let value = <&str as Decode<Postgres>>::decode(value)?;
        let inner = value.as_bytes().try_into()?;

        Ok(Self { inner })
    }
}

impl Type<Postgres> for DbCountryCode {
    #[inline]
    fn type_info() -> <Postgres as sqlx::Database>::TypeInfo {
        PgTypeInfo::with_name("VARCHAR")
    }
}

impl TryFrom<DbCountryCode> for [u8; 2] {
    type Error = ();

    #[inline]
    fn try_from(value: DbCountryCode) -> Result<Self, Self::Error> {
        Ok(value.inner)
    }
}

#[derive(FromRow)]
pub struct UserStatsEntry<V> {
    #[sqlx(rename = "country_code", try_from = "DbCountryCode")]
    pub country: [u8; 2],
    #[sqlx(rename = "username")]
    pub name: String,
    pub value: V,
}

pub trait OsuUserStatsColumn {
    type Stats;
    type Value;

    fn from_stats(stats: &Self::Stats) -> Self::Value;
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

impl UserStatsColumn {
    pub(crate) fn column(self) -> &'static str {
        match self {
            Self::Badges => "badges",
            Self::Comments => "comment_count",
            Self::Followers => "followers",
            Self::ForumPosts => "forum_post_count",
            Self::GraveyardMapsets => "graveyard_mapset_count",
            Self::JoinDate => "join_date",
            Self::KudosuAvailable => "kudosu_available",
            Self::KudosuTotal => "kudosu_total",
            Self::LovedMapsets => "loved_mapset_count",
            Self::Subscribers => "mapping_followers",
            Self::Medals => "medals",
            Self::PlayedMaps => "played_maps",
            Self::RankedMapsets => "ranked_mapset_count",
            Self::Namechanges => "previous_usernames_count",
        }
    }
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
    #[option(name = "Total hits", value = "total_hits")]
    TotalHits,
}

impl UserModeStatsColumn {
    pub(crate) fn column(self) -> Option<&'static str> {
        match self {
            Self::Accuracy => Some("accuracy"),
            Self::AverageHits => None,
            Self::CountSsh => Some("count_ssh"),
            Self::CountSs => Some("count_ss"),
            Self::TotalSs => None,
            Self::CountSh => Some("count_sh"),
            Self::CountS => Some("count_s"),
            Self::TotalS => None,
            Self::CountA => Some("count_a"),
            Self::Level => Some("user_level"),
            Self::MaxCombo => Some("max_combo"),
            Self::Playcount => Some("playcount"),
            Self::Playtime => Some("playtime"),
            Self::Pp => Some("pp"),
            Self::RankCountry => Some("country_rank"),
            Self::RankGlobal => Some("global_rank"),
            Self::ReplaysWatched => Some("replays_watched"),
            Self::ScoreRanked => Some("ranked_score"),
            Self::ScoreTotal => Some("total_score"),
            Self::ScoresFirst => Some("scores_first"),
            Self::TotalHits => Some("total_hits"),
        }
    }
}
