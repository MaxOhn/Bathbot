use rkyv::{
    with::{ArchiveWith, Map, Niche},
    Archive, Deserialize, Serialize,
};
use rkyv_with::ArchiveWith;
use twilight_model::{
    application::interaction::application_command::InteractionMember,
    gateway::payload::incoming::MemberUpdate,
    guild::{Member as TwMember, PartialMember},
    id::{marker::RoleMarker, Id},
    util::ImageHash as TwImageHash,
};

use crate::{
    rkyv_util::NicheDerefAsBox,
    twilight_model::{id::IdRkyv, util::ImageHash},
};

#[derive(Archive, ArchiveWith, Deserialize, Serialize)]
#[archive_with(from(TwMember, InteractionMember, MemberUpdate, PartialMember))]
pub struct Member {
    #[with(Map<ImageHash>)]
    pub avatar: Option<TwImageHash>,
    #[with(Niche)]
    #[archive_with(from(Option<String>), via(NicheDerefAsBox))]
    pub nick: Option<Box<str>>,
    #[with(Map<IdRkyv>)]
    #[archive_with(from(Vec<Id<RoleMarker>>), via(Map<IdRkyv>))]
    pub roles: Vec<Id<RoleMarker>>, // TODO: make box
}

#[cfg(test)]
mod tests {
    use rkyv::with::With;

    use super::{Member, TwMember};

    #[allow(unused)]
    fn test_role(member: &TwMember) {
        let bytes = rkyv::to_bytes::<_, 0>(With::<_, Member>::cast(member)).unwrap();
    }
}
