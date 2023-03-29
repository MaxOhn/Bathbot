use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Deserialize, Fallible, Serialize,
};
use rkyv_with::ArchiveWith;
use twilight_model::{
    channel::permission_overwrite::{
        PermissionOverwrite as TwPermissionOverwrite, PermissionOverwriteType,
    },
    guild::Permissions,
    id::{marker::GenericMarker, Id},
};

use crate::{rkyv_util::FlagsRkyv, twilight_model::id::IdRkyv};

#[derive(Archive, ArchiveWith, Deserialize, Serialize)]
#[archive_with(from(TwPermissionOverwrite))]
pub struct PermissionOverwrite {
    #[with(FlagsRkyv)]
    pub allow: Permissions,
    #[with(FlagsRkyv)]
    pub deny: Permissions,
    #[with(IdRkyv)]
    pub id: Id<GenericMarker>,
    #[with(PermissionOverwriteTypeRkyv)]
    pub kind: PermissionOverwriteType,
}

pub struct PermissionOverwriteTypeRkyv;

impl ArchiveWith<PermissionOverwriteType> for PermissionOverwriteTypeRkyv {
    type Archived = u8;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(
        field: &PermissionOverwriteType,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let byte = match *field {
            PermissionOverwriteType::Member => 0,
            PermissionOverwriteType::Role => 1,
            PermissionOverwriteType::Unknown(other) => other,
            _ => 255,
        };

        <u8 as Archive>::resolve(&byte, pos, resolver, out);
    }
}

impl<S: Fallible + ?Sized> SerializeWith<PermissionOverwriteType, S>
    for PermissionOverwriteTypeRkyv
{
    #[inline]
    fn serialize_with(
        _: &PermissionOverwriteType,
        _: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(())
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<u8, PermissionOverwriteType, D>
    for PermissionOverwriteTypeRkyv
{
    #[inline]
    fn deserialize_with(
        byte: &u8,
        _: &mut D,
    ) -> Result<PermissionOverwriteType, <D as Fallible>::Error> {
        let overwrite_type = match *byte {
            0 => PermissionOverwriteType::Member,
            1 => PermissionOverwriteType::Role,
            other => PermissionOverwriteType::Unknown(other),
        };

        Ok(overwrite_type)
    }
}
