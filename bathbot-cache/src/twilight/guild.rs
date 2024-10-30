use std::time::Duration;

use bathbot_model::twilight::ImageHashRkyv;
use redlight::{
    config::{Cacheable, ICachedGuild},
    rkyv_util::{id::IdRkyv, util::BitflagsRkyv},
    CachedArchive,
};
use rkyv::{
    rancor::{Source, Strategy},
    rend::u64_le,
    util::AlignedVec,
    with::Map,
    Archive, Deserialize, Serialize,
};
use twilight_model::{
    gateway::payload::incoming::GuildUpdate,
    guild::{Guild, Permissions},
    id::{
        marker::{GuildMarker, UserMarker},
        Id,
    },
    util::ImageHash,
};

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

impl ArchivedCachedGuild {
    fn skip_update(&self, update: &GuildUpdate) -> bool {
        ImageHashRkyv::is_eq_opt(self.icon.as_ref(), update.icon.as_ref())
            && self.name.as_ref() == update.name
            && self.owner_id == update.owner_id
            && self.permissions.as_ref().copied().map(u64_le::to_native)
                == update.permissions.as_ref().map(Permissions::bits)
    }
}

impl ICachedGuild<'_> for CachedGuild {
    fn from_guild(guild: &Guild) -> Self {
        Self {
            icon: guild.icon,
            id: guild.id,
            name: Box::from(guild.name.as_str()),
            owner_id: guild.owner_id,
            permissions: guild.permissions,
        }
    }

    fn on_guild_update<E: Source>(
    ) -> Option<fn(&mut CachedArchive<Self>, &GuildUpdate) -> Result<(), E>> {
        Some(|archived, update| {
            if archived.skip_update(update) {
                return Ok(());
            }

            archived
                .update_by_deserializing(
                    |deserialized| {
                        deserialized.icon = update.icon;
                        deserialized.name = Box::from(update.name.as_str());
                        deserialized.owner_id = update.owner_id;
                        deserialized.permissions = update.permissions;
                    },
                    &mut (),
                )
                .map_err(Source::new)
        })
    }
}

impl Cacheable for CachedGuild {
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
