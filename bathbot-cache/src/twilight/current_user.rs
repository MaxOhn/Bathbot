use std::time::Duration;

use bathbot_model::{rkyv_util::StrAsString, twilight::ImageHashRkyv};
use redlight::{
    config::{Cacheable, ICachedCurrentUser},
    rkyv_util::id::IdRkyv,
};
use rkyv::{
    rancor::{Source, Strategy},
    util::AlignedVec,
    with::Map,
    Archive, Serialize,
};
use twilight_model::{
    id::{marker::UserMarker, Id},
    user::CurrentUser,
    util::ImageHash,
};

#[derive(Archive, Serialize)]
pub struct CachedCurrentUser<'a> {
    #[rkyv(with = Map<ImageHashRkyv>)]
    pub avatar: Option<ImageHash>,
    #[rkyv(with = IdRkyv)]
    pub id: Id<UserMarker>,
    #[rkyv(with = StrAsString)]
    pub name: &'a str,
}

impl<'a> ICachedCurrentUser<'a> for CachedCurrentUser<'a> {
    fn from_current_user(current_user: &'a CurrentUser) -> Self {
        Self {
            avatar: current_user.avatar,
            id: current_user.id,
            name: &current_user.name,
        }
    }
}

impl Cacheable for CachedCurrentUser<'_> {
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
