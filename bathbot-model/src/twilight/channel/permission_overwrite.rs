use rkyv::{
    rancor::Fallible,
    with::{ArchiveWith, SerializeWith},
    Archive, Archived, Place, Resolver, Serialize,
};
use twilight_model::{
    channel::permission_overwrite::{PermissionOverwrite, PermissionOverwriteType},
    guild::Permissions,
    id::{marker::GenericMarker, Id},
};

use crate::{rkyv_util::BitflagsRkyv, twilight::id::IdRkyv};

#[derive(Archive, Serialize)]
#[rkyv(remote = PermissionOverwrite, archived = ArchivedPermissionOverwrite)]
pub struct PermissionOverwriteRkyv {
    #[rkyv(with = BitflagsRkyv)]
    pub allow: Permissions,
    #[rkyv(with = BitflagsRkyv)]
    pub deny: Permissions,
    #[rkyv(with = IdRkyv)]
    pub id: Id<GenericMarker>,
    #[rkyv(with = PermissionOverwriteTypeRkyv)]
    pub kind: PermissionOverwriteType,
}

pub struct PermissionOverwriteTypeRkyv;

impl PermissionOverwriteTypeRkyv {
    pub fn deserialize(archived: u8) -> PermissionOverwriteType {
        let overwrite_type = match archived {
            0 => PermissionOverwriteType::Member,
            1 => PermissionOverwriteType::Role,
            other => PermissionOverwriteType::Unknown(other),
        };

        overwrite_type
    }
}

impl ArchiveWith<PermissionOverwriteType> for PermissionOverwriteTypeRkyv {
    type Archived = Archived<u8>;
    type Resolver = Resolver<u8>;

    fn resolve_with(
        field: &PermissionOverwriteType,
        resolver: Self::Resolver,
        out: Place<Self::Archived>,
    ) {
        let byte = match field {
            PermissionOverwriteType::Member => 0,
            PermissionOverwriteType::Role => 1,
            PermissionOverwriteType::Unknown(other) => *other,
            _ => 255,
        };

        byte.resolve(resolver, out);
    }
}

impl<S: Fallible + ?Sized> SerializeWith<PermissionOverwriteType, S>
    for PermissionOverwriteTypeRkyv
{
    fn serialize_with(_: &PermissionOverwriteType, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}
