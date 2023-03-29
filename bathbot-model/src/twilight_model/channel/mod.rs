use rkyv::{
    with::{ArchiveWith, Map, Niche},
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

use crate::rkyv_util::NicheDerefAsBox;

use super::id::{IdNiche, IdRkyv};

mod channel_type;
mod permission_overwrite;

pub use self::{
    channel_type::ChannelTypeRkyv,
    permission_overwrite::{
        ArchivedPermissionOverwrite, PermissionOverwrite, PermissionOverwriteResolver,
        PermissionOverwriteTypeRkyv,
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
    #[archive_with(from(Option<Vec<TwPermissionOverwrite>>), via(Map<Map<PermissionOverwrite>>))]
    pub permission_overwrites: Option<Vec<PermissionOverwrite>>, // TODO: make box
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
