use std::time::Duration;

use bathbot_model::rkyv_util::StrAsString;
use redlight::{
    config::{Cacheable, ICachedRole},
    rkyv_util::{id::IdRkyv, util::BitflagsRkyv},
};
use rkyv::{
    rancor::{Source, Strategy},
    util::AlignedVec,
    Archive, Serialize,
};
use twilight_model::{
    guild::{Permissions, Role},
    id::{marker::RoleMarker, Id},
};

#[derive(Archive, Serialize)]
pub struct CachedRole<'a> {
    #[rkyv(with = IdRkyv)]
    pub id: Id<RoleMarker>,
    #[rkyv(with = StrAsString)]
    pub name: &'a str,
    #[rkyv(with = BitflagsRkyv)]
    pub permissions: Permissions,
}

impl<'a> ICachedRole<'a> for CachedRole<'a> {
    fn from_role(role: &'a Role) -> Self {
        Self {
            id: role.id,
            name: &role.name,
            permissions: role.permissions,
        }
    }
}

impl Cacheable for CachedRole<'_> {
    type Bytes = AlignedVec<8>;

    fn expire() -> Option<Duration> {
        None
    }

    fn serialize_one<E: Source>(&self) -> Result<Self::Bytes, E> {
        let mut serializer = AlignedVec::default();
        let strategy = Strategy::wrap(&mut serializer);
        rkyv::api::serialize_using(self, strategy)?;

        Ok(serializer)
    }
}
