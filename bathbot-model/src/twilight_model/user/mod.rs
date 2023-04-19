mod current_user;

use rkyv::{
    with::{ArchiveWith, Map},
    Archive, Deserialize, Serialize,
};
use rkyv_with::ArchiveWith;
use twilight_model::{
    id::{marker::UserMarker, Id},
    user::User as TwUser,
    util::ImageHash as TwImageHash,
};

pub use self::current_user::CurrentUser;
use super::{id::IdRkyv, util::ImageHash};
use crate::rkyv_util::DerefAsBox;

#[derive(Archive, ArchiveWith, Deserialize, Serialize)]
#[archive_with(from(TwUser))]
pub struct User {
    #[with(Map<ImageHash>)]
    pub avatar: Option<TwImageHash>,
    pub bot: bool,
    pub discriminator: u16,
    #[with(IdRkyv)]
    pub id: Id<UserMarker>,
    #[archive_with(from(String), via(DerefAsBox))]
    pub name: Box<str>,
}

#[cfg(test)]
mod tests {
    use rkyv::with::With;

    use super::{TwUser, User};

    #[allow(unused)]
    fn test_user(user: &TwUser) {
        let bytes = rkyv::to_bytes::<_, 0>(With::<_, User>::cast(user)).unwrap();
    }
}
