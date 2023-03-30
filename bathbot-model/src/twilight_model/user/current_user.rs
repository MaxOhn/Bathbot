use rkyv::{
    with::{ArchiveWith, Map},
    Archive, Deserialize, Serialize,
};
use rkyv_with::ArchiveWith;
use twilight_model::{
    id::{marker::UserMarker, Id},
    user::CurrentUser as TwCurrentUser,
    util::ImageHash as TwImageHash,
};

use crate::{
    rkyv_util::DerefAsBox,
    twilight_model::{id::IdRkyv, util::ImageHash},
};

#[derive(Archive, ArchiveWith, Deserialize, Serialize)]
#[archive_with(from(TwCurrentUser))]
pub struct CurrentUser {
    #[with(Map<ImageHash>)]
    pub avatar: Option<TwImageHash>,
    pub discriminator: u16,
    #[with(IdRkyv)]
    pub id: Id<UserMarker>,
    #[archive_with(from(String), via(DerefAsBox))]
    pub name: Box<str>,
}

#[cfg(test)]
mod tests {
    use rkyv::with::With;

    use super::{CurrentUser, TwCurrentUser};

    #[allow(unused)]
    fn test_current_user(user: &TwCurrentUser) {
        let _ = rkyv::to_bytes::<_, 0>(With::<_, CurrentUser>::cast(user)).unwrap();
    }
}
