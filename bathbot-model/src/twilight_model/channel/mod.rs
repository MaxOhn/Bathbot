use rkyv::{
    with::{ArchiveWith, Niche},
    Archive, Deserialize, Serialize,
};
use rkyv_with::ArchiveWith;
use twilight_model::{
    channel::{
        permission_overwrite::PermissionOverwrite as TwPermissionOverwrite, Channel as TwChannel,
        ChannelType,
    },
    id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    },
};

use super::id::{IdNiche, IdRkyv};
use crate::rkyv_util::NicheDerefAsBox;

mod channel_type;
mod permission_overwrite;

pub use self::{
    channel_type::ChannelTypeRkyv,
    permission_overwrite::{
        ArchivedPermissionOverwrite, PermissionOverwrite, PermissionOverwriteOptionVec,
        PermissionOverwriteResolver, PermissionOverwriteTypeRkyv,
    },
};

#[derive(Archive, ArchiveWith, Deserialize, Serialize)]
#[archive_with(from(TwChannel))]
pub struct Channel {
    #[with(IdNiche)]
    pub guild_id: Option<Id<GuildMarker>>,
    #[with(IdRkyv)]
    pub id: Id<ChannelMarker>,
    #[with(ChannelTypeRkyv)]
    pub kind: ChannelType,
    #[with(Niche)]
    #[archive_with(from(Option<String>), via(NicheDerefAsBox))]
    pub name: Option<Box<str>>,
    #[with(IdNiche)]
    pub parent_id: Option<Id<ChannelMarker>>,
    #[with(Niche)]
    #[archive_with(from(Option<Vec<TwPermissionOverwrite>>), via(PermissionOverwriteOptionVec))]
    pub permission_overwrites: Option<Box<[PermissionOverwrite]>>,
    pub position: Option<i32>,
}

#[cfg(test)]
mod tests {
    use rkyv::with::With;

    use super::{Channel, TwChannel};

    #[allow(unused)]
    fn test_role(channel: &TwChannel) {
        let bytes = rkyv::to_bytes::<_, 0>(With::<_, Channel>::cast(channel)).unwrap();
    }
}
