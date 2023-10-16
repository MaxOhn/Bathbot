use rkyv::{
    with::{ArchiveWith, Map, Niche},
    Archive, Deserialize,
};
use rkyv_with::ArchiveWith;
use rosu_v2::{
    model::{
        ranking::Rankings as RosuRankings,
        user::{User as RosuUser, UserStatistics as RosuUserStatistics},
    },
    prelude::{CountryCode, Username},
};

use super::user::UserStatistics;
use crate::rkyv_util::{DerefAsBox, DerefAsString, NicheDerefAsBox};

#[derive(Archive, ArchiveWith, Deserialize)]
#[archive_with(from(RosuRankings))]
pub struct Rankings {
    #[archive_with(from(Vec<RosuUser>), via(Map<RankingsUser>))]
    pub ranking: Vec<RankingsUser>,
    pub total: u32,
}

#[derive(Archive, ArchiveWith, Deserialize)]
#[archive_with(from(RosuUser))]
pub struct RankingsUser {
    #[archive_with(from(String), via(DerefAsBox))]
    pub avatar_url: Box<str>,
    #[with(DerefAsString)]
    pub country_code: CountryCode,
    #[with(Niche)]
    #[archive_with(from(Option<String>), via(NicheDerefAsBox))]
    pub country: Option<Box<str>>,
    pub user_id: u32,
    #[with(DerefAsString)]
    pub username: Username,
    #[archive_with(from(Option<RosuUserStatistics>), via(Map<UserStatistics>))]
    pub statistics: Option<UserStatistics>,
}

impl From<RosuUser> for RankingsUser {
    #[inline]
    fn from(user: RosuUser) -> Self {
        Self {
            avatar_url: user.avatar_url.into_boxed_str(),
            country_code: user.country_code,
            country: user.country.map(String::into_boxed_str),
            user_id: user.user_id,
            username: user.username,
            statistics: user.statistics.map(UserStatistics::from),
        }
    }
}
