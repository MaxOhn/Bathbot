use std::mem;

use rkyv::{
    niche::option_box::{ArchivedOptionBox, OptionBoxResolver},
    ser::{ScratchSpace, Serializer},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, ArchiveUnsized, Deserialize, Fallible, Serialize,
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

pub struct PermissionOverwriteOptionVec;

impl ArchiveWith<Option<Vec<TwPermissionOverwrite>>> for PermissionOverwriteOptionVec {
    type Archived = ArchivedOptionBox<<[PermissionOverwrite] as ArchiveUnsized>::Archived>;
    type Resolver = OptionBoxResolver<<[PermissionOverwrite] as ArchiveUnsized>::MetadataResolver>;

    unsafe fn resolve_with(
        field: &Option<Vec<TwPermissionOverwrite>>,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let opt = field.as_ref().map(Vec::as_slice).map(tw_to_native);
        ArchivedOptionBox::resolve_from_option(opt, pos, resolver, out);
    }
}

impl<S: Serializer + ScratchSpace + Fallible> SerializeWith<Option<Vec<TwPermissionOverwrite>>, S>
    for PermissionOverwriteOptionVec
{
    fn serialize_with(
        field: &Option<Vec<TwPermissionOverwrite>>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        let opt = field.as_ref().map(Vec::as_slice).map(tw_to_native);

        ArchivedOptionBox::serialize_from_option(opt, serializer)
    }
}

fn tw_to_native(tw: &[TwPermissionOverwrite]) -> &[PermissionOverwrite] {
    const _: () = {
        fn assert_eq(permission_overwrite: TwPermissionOverwrite) {
            let TwPermissionOverwrite {
                allow,
                deny,
                id,
                kind,
            } = permission_overwrite;
            let _ = PermissionOverwrite {
                allow,
                deny,
                id,
                kind,
            };
        }
    };

    // SAFETY: field equality is checked during compile-time
    unsafe { mem::transmute(tw) }
}
