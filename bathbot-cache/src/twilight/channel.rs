use std::time::Duration;

use redlight::{
    config::{Cacheable, ICachedChannel},
    rkyv_util::id::{IdRkyv, IdRkyvMap},
    CachedArchive,
};
use rkyv::{
    rancor::{Source, Strategy},
    ser::Serializer,
    util::AlignedVec,
    with::Map,
    Archive, Serialize,
};
use twilight_model::{
    channel::{permission_overwrite::PermissionOverwrite, Channel},
    gateway::payload::incoming::ChannelPinsUpdate,
    id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    },
};

use super::permission_overwrite::PermissionOverwriteRkyv;

#[derive(Archive, Serialize)]
pub struct CachedChannel {
    #[rkyv(with = IdRkyvMap)]
    pub guild_id: Option<Id<GuildMarker>>,
    #[rkyv(with = IdRkyv)]
    pub id: Id<ChannelMarker>,
    #[rkyv(with = Map<Map<PermissionOverwriteRkyv>>)]
    pub permission_overwrites: Option<Vec<PermissionOverwrite>>,
}

impl ICachedChannel<'_> for CachedChannel {
    fn from_channel(channel: &'_ Channel) -> Self {
        Self {
            guild_id: channel.guild_id,
            id: channel.id,
            permission_overwrites: channel.permission_overwrites.clone(),
        }
    }

    fn on_pins_update<E: Source>(
    ) -> Option<fn(&mut CachedArchive<Self>, &ChannelPinsUpdate) -> Result<(), E>> {
        None
    }
}

impl Cacheable for CachedChannel {
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
