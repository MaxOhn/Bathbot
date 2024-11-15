use permission_overwrite::PermissionOverwriteRkyv;
use rkyv::{
    munge::munge,
    rancor::Fallible,
    ser::{Allocator, Writer},
    with::{ArchiveWith, Map, SerializeWith},
    Archive, Place, Serialize,
};
use twilight_model::{
    channel::{permission_overwrite::PermissionOverwrite, Channel},
    id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    },
};

use super::id::{IdRkyv, IdRkyvMap};

mod permission_overwrite;

pub use self::permission_overwrite::{ArchivedPermissionOverwrite, PermissionOverwriteTypeRkyv};

#[derive(Archive, Serialize)]
pub struct CachedChannel {
    #[rkyv(with = IdRkyvMap)]
    pub guild_id: Option<Id<GuildMarker>>,
    #[rkyv(with = IdRkyv)]
    pub id: Id<ChannelMarker>,
    #[rkyv(with = Map<Map<PermissionOverwriteRkyv>>)]
    pub permission_overwrites: Option<Vec<PermissionOverwrite>>,
}

impl ArchiveWith<Channel> for CachedChannel {
    type Archived = ArchivedCachedChannel;
    type Resolver = CachedChannelResolver;

    fn resolve_with(channel: &Channel, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedCachedChannel { guild_id, id, permission_overwrites } = out);
        IdRkyvMap::resolve_with(&channel.guild_id, resolver.guild_id, guild_id);
        IdRkyv::resolve_with(&channel.id, resolver.id, id);
        Map::<Map<PermissionOverwriteRkyv>>::resolve_with(
            &channel.permission_overwrites,
            resolver.permission_overwrites,
            permission_overwrites,
        );
    }
}

impl<S: Fallible + Writer + Allocator + ?Sized> SerializeWith<Channel, S> for CachedChannel {
    fn serialize_with(channel: &Channel, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(CachedChannelResolver {
            guild_id: IdRkyvMap::serialize_with(&channel.guild_id, serializer)?,
            id: IdRkyv::serialize_with(&channel.id, serializer)?,
            permission_overwrites: Map::<Map<PermissionOverwriteRkyv>>::serialize_with(
                &channel.permission_overwrites,
                serializer,
            )?,
        })
    }
}
