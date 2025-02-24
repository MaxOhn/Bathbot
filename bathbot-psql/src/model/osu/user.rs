use bathbot_model::{UserModeStatsColumn, UserStatsColumn};
use sqlx::{Database, Decode, FromRow, Postgres, Type, error::BoxDynError, postgres::PgTypeInfo};

struct DbCountryCode {
    inner: [u8; 2],
}

impl<'r> Decode<'r, Postgres> for DbCountryCode {
    #[inline]
    fn decode(value: <Postgres as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
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

// Be sure to keep structure in sync with
// `bathbot_model::user_stats::UserStatsEntry`!
#[derive(FromRow)]
pub struct DbUserStatsEntry<V> {
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
pub(crate) trait OsuUserStatsColumnName: Copy {
    type Name;

    fn column(self) -> Self::Name;
}

impl OsuUserStatsColumnName for UserStatsColumn {
    type Name = &'static str;

    #[inline]
    fn column(self) -> &'static str {
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

impl OsuUserStatsColumnName for UserModeStatsColumn {
    type Name = Option<&'static str>;

    #[inline]
    fn column(self) -> Option<&'static str> {
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
            Self::PpPerMonth => None,
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
