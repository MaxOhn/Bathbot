use std::time::Duration;

use bathbot_model::twilight::ImageHashRkyv;
use redlight::{
    config::{Cacheable, ICachedMember},
    rkyv_util::id::IdRkyvMap,
    CachedArchive,
};
use rkyv::{
    rancor::{Source, Strategy},
    ser::Serializer,
    util::AlignedVec,
    with::{Map, Niche},
    Archive, Deserialize, Serialize,
};
use twilight_model::{
    gateway::payload::incoming::MemberUpdate,
    guild::{Member, PartialMember},
    id::{
        marker::{GuildMarker, RoleMarker},
        Id,
    },
    util::ImageHash,
};

#[derive(Archive, Serialize, Deserialize)]
pub struct CachedMember {
    #[rkyv(with = Map<ImageHashRkyv>)]
    pub avatar: Option<ImageHash>,
    #[rkyv(with = Niche)]
    pub nick: Option<Box<str>>,
    #[rkyv(with = IdRkyvMap)]
    pub roles: Box<[Id<RoleMarker>]>,
}

impl ArchivedCachedMember {
    fn skip_update(
        &self,
        avatar: Option<&ImageHash>,
        nick: Option<&str>,
        roles: &[Id<RoleMarker>],
    ) -> bool {
        ImageHashRkyv::is_eq_opt(self.avatar.as_ref(), avatar)
            && self.nick.as_ref().map(<_>::as_ref) == nick
            && self.roles.as_ref() == roles
    }
}

impl ICachedMember<'_> for CachedMember {
    fn from_member(_: Id<GuildMarker>, member: &Member) -> Self {
        Self {
            avatar: member.avatar,
            nick: member.nick.as_deref().map(Box::from),
            roles: Box::from(member.roles.as_slice()),
        }
    }

    fn update_via_partial<E: Source>(
    ) -> Option<fn(&mut CachedArchive<Self>, &PartialMember) -> Result<(), E>> {
        Some(|archived, update| {
            if archived.skip_update(
                update.avatar.as_ref(),
                update.nick.as_deref(),
                &update.roles,
            ) {
                return Ok(());
            }

            archived
                .update_by_deserializing(
                    |deserialized| {
                        deserialized.avatar = update.avatar;
                        deserialized.nick = update.nick.as_deref().map(Box::from);
                        deserialized.roles = Box::from(update.roles.as_slice());
                    },
                    &mut (),
                )
                .map_err(Source::new)
        })
    }

    fn on_member_update<E: Source>(
    ) -> Option<fn(&mut CachedArchive<Self>, &MemberUpdate) -> Result<(), E>> {
        Some(|archived, update| {
            if archived.skip_update(
                update.avatar.as_ref(),
                update.nick.as_deref(),
                &update.roles,
            ) {
                return Ok(());
            }

            archived
                .update_by_deserializing(
                    |deserialized| {
                        deserialized.avatar = update.avatar;
                        deserialized.nick = update.nick.as_deref().map(Box::from);
                        deserialized.roles = Box::from(update.roles.as_slice());
                    },
                    &mut (),
                )
                .map_err(Source::new)
        })
    }
}

impl Cacheable for CachedMember {
    type Bytes = AlignedVec<8>;

    fn expire() -> Option<Duration> {
        None
    }

    fn serialize_one<E: Source>(&self) -> Result<Self::Bytes, E> {
        rkyv::util::with_arena(|arena| {
            let mut serializer = Serializer::new(AlignedVec::default(), arena.acquire(), ());
            let strategy = Strategy::wrap(&mut serializer);
            rkyv::api::serialize_using(self, strategy)?;

            Ok(serializer.into_writer())
        })
    }
}
