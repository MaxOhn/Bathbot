mod member;
mod role;

use rkyv::{
    Archive, Deserialize, Place, Serialize,
    munge::munge,
    rancor::Fallible,
    ser::Writer,
    with::{ArchiveWith, Map, SerializeWith},
};
use twilight_model::{
    guild::{Guild, PartialGuild, Permissions},
    id::{
        Id,
        marker::{GuildMarker, UserMarker},
    },
    util::ImageHash,
};

pub use self::{
    member::{ArchivedCachedMember, CachedMember},
    role::{ArchivedCachedRole, CachedRole},
};
use super::{id::IdRkyv, util::ImageHashRkyv};
use crate::rkyv_util::{BitflagsRkyv, DerefAsBox};

#[derive(Archive, Serialize, Deserialize)]
pub struct CachedGuild {
    #[rkyv(with = Map<ImageHashRkyv>)]
    pub icon: Option<ImageHash>,
    #[rkyv(with = IdRkyv)]
    pub id: Id<GuildMarker>,
    pub name: Box<str>,
    #[rkyv(with = IdRkyv)]
    pub owner_id: Id<UserMarker>,
    #[rkyv(with = Map<BitflagsRkyv>)]
    pub permissions: Option<Permissions>,
}

macro_rules! impl_with {
    ( $ty:ty ) => {
        impl ArchiveWith<$ty> for CachedGuild {
            type Archived = ArchivedCachedGuild;
            type Resolver = CachedGuildResolver;

            fn resolve_with(guild: &$ty, resolver: Self::Resolver, out: Place<Self::Archived>) {
                munge!(let ArchivedCachedGuild { icon, id, name, owner_id, permissions } = out);
                Map::<ImageHashRkyv>::resolve_with(&guild.icon, resolver.icon, icon);
                IdRkyv::resolve_with(&guild.id, resolver.id, id);
                DerefAsBox::resolve_with(&guild.name, resolver.name, name);
                IdRkyv::resolve_with(&guild.owner_id, resolver.owner_id, owner_id);
                Map::<BitflagsRkyv>::resolve_with(&guild.permissions, resolver.permissions, permissions);
            }
        }

        impl<S: Fallible + Writer + ?Sized> SerializeWith<$ty, S> for CachedGuild {
            fn serialize_with(guild: &$ty, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
                Ok(CachedGuildResolver {
                    icon: Map::<ImageHashRkyv>::serialize_with(&guild.icon, serializer)?,
                    id: IdRkyv::serialize_with(&guild.id, serializer)?,
                    name: DerefAsBox::serialize_with(&guild.name, serializer)?,
                    owner_id: IdRkyv::serialize_with(&guild.owner_id, serializer)?,
                    permissions: Map::<BitflagsRkyv>::serialize_with(&guild.permissions, serializer)?,
                })
            }
        }
    };
}

impl_with!(Guild);
impl_with!(PartialGuild);
