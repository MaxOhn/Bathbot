use rkyv::{with::ArchiveWith, Archive, Deserialize, Serialize};
use rkyv_with::ArchiveWith;
use twilight_model::{
    guild::{Permissions, Role as TwRole},
    id::{marker::RoleMarker, Id},
};

use crate::{
    rkyv_util::DerefAsBox,
    twilight_model::{id::IdRkyv, util::FlagsRkyv},
};

#[derive(Archive, ArchiveWith, Deserialize, Serialize)]
#[archive_with(from(TwRole))]
pub struct Role {
    #[with(IdRkyv)]
    pub id: Id<RoleMarker>,
    #[archive_with(from(String), via(DerefAsBox))]
    pub name: Box<str>,
    #[with(FlagsRkyv)]
    pub permissions: Permissions,
    pub position: i64,
}

#[cfg(test)]
mod tests {
    use rkyv::with::With;

    use super::{Role, TwRole};

    #[allow(unused)]
    fn test_role(role: &TwRole) {
        let bytes = rkyv::to_bytes::<_, 0>(With::<_, Role>::cast(role)).unwrap();
    }
}
