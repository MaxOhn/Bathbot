use std::time::Duration;

use bathbot_model::twilight::ImageHashRkyv;
use redlight::{
    config::{Cacheable, ICachedUser},
    rkyv_util::id::IdRkyv,
    CachedArchive,
};
use rkyv::{
    rancor::{Source, Strategy},
    util::AlignedVec,
    with::Map,
    Archive, Deserialize, Serialize,
};
use twilight_model::{
    gateway::payload::incoming::invite_create::PartialUser,
    id::{marker::UserMarker, Id},
    user::User,
    util::ImageHash,
};

#[derive(Archive, Serialize, Deserialize)]
pub struct CachedUser {
    #[rkyv(with = Map<ImageHashRkyv>)]
    pub avatar: Option<ImageHash>,
    pub bot: bool,
    #[rkyv(with = IdRkyv)]
    pub id: Id<UserMarker>,
    pub name: Box<str>,
}

impl ArchivedCachedUser {
    fn skip_update(&self, update: &PartialUser) -> bool {
        self.name.as_ref() == update.username
            && ImageHashRkyv::is_eq_opt(self.avatar.as_ref(), update.avatar.as_ref())
    }
}

impl ICachedUser<'_> for CachedUser {
    fn from_user(user: &'_ User) -> Self {
        Self {
            avatar: user.avatar,
            bot: user.bot,
            id: user.id,
            name: Box::from(user.name.as_str()),
        }
    }

    fn update_via_partial<E: Source>(
    ) -> Option<fn(&mut CachedArchive<Self>, &PartialUser) -> Result<(), E>> {
        Some(|archived, update| {
            if archived.skip_update(update) {
                return Ok(());
            }

            archived
                .update_by_deserializing(
                    |deserialized| {
                        deserialized.avatar = update.avatar;
                        deserialized.name = Box::from(update.username.as_str());
                    },
                    &mut (),
                )
                .map_err(Source::new)
        })
    }
}

impl Cacheable for CachedUser {
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
