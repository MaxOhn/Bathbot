mod member;
mod permissions;
mod role;

use rkyv::{
    with::{ArchiveWith, Map},
    Archive, Deserialize, Serialize,
};
use rkyv_with::ArchiveWith;
use twilight_model::{
    guild::{Guild as TwGuild, PartialGuild, Permissions},
    id::{
        marker::{GuildMarker, UserMarker},
        Id,
    },
    util::ImageHash as TwImageHash,
};

pub use self::{
    member::{ArchivedMember, Member, MemberResolver},
    role::{ArchivedRole, Role, RoleResolver},
};
use super::{id::IdRkyv, util::ImageHash};
use crate::rkyv_util::{DerefAsBox, FlagsRkyv};

#[derive(Archive, ArchiveWith, Deserialize, Serialize)]
#[archive_with(from(TwGuild, PartialGuild))]
pub struct Guild {
    #[with(Map<ImageHash>)]
    pub icon: Option<TwImageHash>,
    #[with(IdRkyv)]
    pub id: Id<GuildMarker>,
    #[archive_with(from(String), via(DerefAsBox))]
    pub name: Box<str>,
    #[with(IdRkyv)]
    pub owner_id: Id<UserMarker>,
    #[with(Map<FlagsRkyv>)]
    pub permissions: Option<Permissions>,
}

#[cfg(test)]
mod tests {
    use rkyv::with::With;

    use super::{Guild, TwGuild};

    #[allow(unused)]
    fn test_role(guild: &TwGuild) {
        let bytes = rkyv::to_bytes::<_, 0>(With::<_, Guild>::cast(guild)).unwrap();
    }
}
