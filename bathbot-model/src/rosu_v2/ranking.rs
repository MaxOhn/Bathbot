use rkyv::{with::Map, Archive, Serialize};
use rosu_v2::prelude::{
    CountryCode, User as RosuUser, UserStatistics as RosuUserStatistics, Username,
};

use super::user::UserStatistics;
use crate::rkyv_util::{DerefAsString, NicheDerefAsBox};

#[derive(Archive, Serialize)]
#[rkyv(remote = rosu_v2::prelude::Rankings)]
pub struct Rankings {
    #[rkyv(with = Map<RankingsUser>)]
    pub ranking: Vec<RosuUser>,
    pub total: u32,
}

#[derive(Archive, Serialize)]
#[rkyv(remote = RosuUser)]
pub struct RankingsUser {
    pub avatar_url: String,
    #[rkyv(with = DerefAsString)]
    pub country_code: CountryCode,
    #[rkyv(with = NicheDerefAsBox)]
    pub country: Option<String>,
    pub user_id: u32,
    #[rkyv(with = DerefAsString)]
    pub username: Username,
    #[rkyv(with = Map<UserStatistics>)]
    pub statistics: Option<RosuUserStatistics>,
}
