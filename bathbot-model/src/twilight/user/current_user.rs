use rkyv::{
    munge::munge,
    rancor::{Fallible, Source},
    ser::Writer,
    with::{ArchiveWith, Map, SerializeWith},
    Archive, Place, Serialize,
};
use twilight_model::{
    id::{marker::UserMarker, Id},
    user::CurrentUser,
    util::ImageHash,
};

use crate::{
    rkyv_util::StrAsString,
    twilight::{id::IdRkyv, util::ImageHashRkyv},
};

#[derive(Archive, Serialize)]
pub struct CachedCurrentUser<'a> {
    #[rkyv(with = Map<ImageHashRkyv>)]
    pub avatar: Option<ImageHash>,
    #[rkyv(with = IdRkyv)]
    pub id: Id<UserMarker>,
    #[rkyv(with = StrAsString)]
    pub name: &'a str,
}

impl<'a> ArchiveWith<CurrentUser> for CachedCurrentUser<'a> {
    type Archived = ArchivedCachedCurrentUser<'a>;
    type Resolver = CachedCurrentUserResolver<'a>;

    #[allow(clippy::unit_arg)]
    fn resolve_with(user: &CurrentUser, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedCachedCurrentUser { avatar, id, name } = out);
        Map::<ImageHashRkyv>::resolve_with(&user.avatar, resolver.avatar, avatar);
        IdRkyv::resolve_with(&user.id, resolver.id, id);
        user.name.resolve(resolver.name, name);
    }
}

impl<S: Fallible<Error: Source> + Writer + ?Sized> SerializeWith<CurrentUser, S>
    for CachedCurrentUser<'_>
{
    fn serialize_with(user: &CurrentUser, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(CachedCurrentUserResolver {
            avatar: Map::<ImageHashRkyv>::serialize_with(&user.avatar, serializer)?,
            id: IdRkyv::serialize_with(&user.id, serializer)?,
            name: user.name.serialize(serializer)?,
        })
    }
}
