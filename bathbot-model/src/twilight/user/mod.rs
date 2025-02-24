use rkyv::{
    Archive, Deserialize, Place, Serialize,
    munge::munge,
    rancor::Fallible,
    ser::Writer,
    with::{ArchiveWith, Map, SerializeWith},
};
use twilight_model::{
    id::{Id, marker::UserMarker},
    user::User,
    util::ImageHash,
};

pub use self::current_user::{ArchivedCachedCurrentUser, CachedCurrentUser};
use super::{id::IdRkyv, util::ImageHashRkyv};
use crate::rkyv_util::DerefAsBox;

mod current_user;

#[derive(Archive, Serialize, Deserialize)]
pub struct CachedUser {
    #[rkyv(with = Map<ImageHashRkyv>)]
    pub avatar: Option<ImageHash>,
    pub bot: bool,
    #[rkyv(with = IdRkyv)]
    pub id: Id<UserMarker>,
    pub name: Box<str>,
}

impl ArchiveWith<User> for CachedUser {
    type Archived = ArchivedCachedUser;
    type Resolver = CachedUserResolver;

    #[allow(clippy::unit_arg)]
    fn resolve_with(user: &User, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedCachedUser { avatar, bot, id, name } = out);
        Map::<ImageHashRkyv>::resolve_with(&user.avatar, resolver.avatar, avatar);
        user.bot.resolve(resolver.bot, bot);
        IdRkyv::resolve_with(&user.id, resolver.id, id);
        DerefAsBox::resolve_with(&user.name, resolver.name, name);
    }
}

impl<S: Fallible + Writer + ?Sized> SerializeWith<User, S> for CachedUser {
    fn serialize_with(user: &User, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(CachedUserResolver {
            avatar: Map::<ImageHashRkyv>::serialize_with(&user.avatar, serializer)?,
            bot: user.bot.serialize(serializer)?,
            id: IdRkyv::serialize_with(&user.id, serializer)?,
            name: DerefAsBox::serialize_with(&user.name, serializer)?,
        })
    }
}
