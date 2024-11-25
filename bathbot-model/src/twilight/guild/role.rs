use rkyv::{
    munge::munge,
    rancor::{Fallible, Source},
    ser::Writer,
    with::{ArchiveWith, SerializeWith},
    Archive, Place, Serialize,
};
use twilight_model::{
    guild::{Permissions, Role},
    id::{marker::RoleMarker, Id},
};

use crate::{
    rkyv_util::{BitflagsRkyv, StrAsString},
    twilight::id::IdRkyv,
};

#[derive(Archive, Serialize)]
pub struct CachedRole<'a> {
    #[rkyv(with = IdRkyv)]
    pub id: Id<RoleMarker>,
    #[rkyv(with = StrAsString)]
    pub name: &'a str,
    #[rkyv(with = BitflagsRkyv)]
    pub permissions: Permissions,
}

impl<'a> ArchiveWith<Role> for CachedRole<'a> {
    type Archived = ArchivedCachedRole<'a>;
    type Resolver = CachedRoleResolver<'a>;

    #[allow(clippy::unit_arg)]
    fn resolve_with(role: &Role, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedCachedRole { id, name, permissions } = out);
        IdRkyv::resolve_with(&role.id, resolver.id, id);
        role.name.resolve(resolver.name, name);
        BitflagsRkyv::resolve_with(&role.permissions, resolver.permissions, permissions);
    }
}

impl<S: Fallible<Error: Source> + Writer + ?Sized> SerializeWith<Role, S> for CachedRole<'_> {
    fn serialize_with(role: &Role, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(CachedRoleResolver {
            id: IdRkyv::serialize_with(&role.id, serializer)?,
            name: role.name.serialize(serializer)?,
            permissions: BitflagsRkyv::serialize_with(&role.permissions, serializer)?,
        })
    }
}
