use std::num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8};

use rkyv::{
    niche::option_nonzero::{
        ArchivedOptionNonZeroU16, ArchivedOptionNonZeroU32, ArchivedOptionNonZeroU64,
        ArchivedOptionNonZeroU8,
    },
    rancor::Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Place,
};

pub struct AsNonZero;

macro_rules! impl_as_non_zero {
    ($ar:ty, $nz:ty, $ne:ty) => {
        impl ArchiveWith<Option<$ne>> for AsNonZero {
            type Archived = $ar;
            type Resolver = ();

            #[inline]
            fn resolve_with(field: &Option<$ne>, _: Self::Resolver, out: Place<Self::Archived>) {
                let opt = field.and_then(<$nz>::new);
                <$ar>::resolve_from_option(opt, out);
            }
        }

        impl<S: Fallible + ?Sized> SerializeWith<Option<$ne>, S> for AsNonZero {
            #[inline]
            fn serialize_with(
                _: &Option<$ne>,
                _: &mut S,
            ) -> Result<Self::Resolver, <S as Fallible>::Error> {
                Ok(())
            }
        }

        impl<D: Fallible + ?Sized> DeserializeWith<$ar, Option<$ne>, D> for AsNonZero {
            #[inline]
            fn deserialize_with(
                field: &$ar,
                _: &mut D,
            ) -> Result<Option<$ne>, <D as Fallible>::Error> {
                Ok(field.as_ref().copied().map(<$nz>::from).map(<$nz>::get))
            }
        }
    };
}

impl_as_non_zero!(ArchivedOptionNonZeroU8, NonZeroU8, u8);
impl_as_non_zero!(ArchivedOptionNonZeroU16, NonZeroU16, u16);
impl_as_non_zero!(ArchivedOptionNonZeroU32, NonZeroU32, u32);
impl_as_non_zero!(ArchivedOptionNonZeroU64, NonZeroU64, u64);
