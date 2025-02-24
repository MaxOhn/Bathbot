use rkyv::{
    Archive, Serialize,
    niche::niching::NaN,
    with::{Map, MapNiche},
};
use rosu_v2::prelude::{CountryCode, Rankings, User, UserStatistics, Username};

use super::user::UserStatisticsRkyv;
use crate::rkyv_util::{DerefAsString, NicheDerefAsBox};

#[derive(Archive, Serialize)]
#[rkyv(remote = Rankings, archived = ArchivedRankings)]
pub struct RankingsRkyv {
    #[rkyv(with = Map<RankingsUserRkyv>)]
    pub ranking: Vec<User>,
    pub total: u32,
}

#[derive(Archive, Serialize)]
#[rkyv(remote = User, archived = ArchivedRankingsUser)]
pub struct RankingsUserRkyv {
    pub avatar_url: String,
    #[rkyv(with = DerefAsString)]
    pub country_code: CountryCode,
    #[rkyv(with = NicheDerefAsBox)]
    pub country: Option<String>,
    pub user_id: u32,
    #[rkyv(with = DerefAsString)]
    pub username: Username,
    #[rkyv(with = MapNiche<UserStatisticsRkyv, NaN>)]
    pub statistics: Option<UserStatistics>,
}
