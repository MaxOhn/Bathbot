use std::num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8};

use rkyv::{
    niche::{niched_option::NichedOption, niching::Zero},
    rancor::Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archived, Place,
};

pub struct AsNonZero;

macro_rules! impl_as_non_zero {
    ($nz:ty, $ne:ty) => {
        impl ArchiveWith<Option<$ne>> for AsNonZero {
            type Archived = NichedOption<Archived<$nz>, Zero>;
            type Resolver = ();

            #[inline]
            fn resolve_with(field: &Option<$ne>, _: Self::Resolver, out: Place<Self::Archived>) {
                let opt = field.and_then(<$nz>::new);
                NichedOption::resolve_from_option(opt.as_ref(), Some(()), out);
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

        impl<D: Fallible + ?Sized>
            DeserializeWith<NichedOption<Archived<$nz>, Zero>, Option<$ne>, D> for AsNonZero
        {
            #[inline]
            fn deserialize_with(
                field: &NichedOption<Archived<$nz>, Zero>,
                _: &mut D,
            ) -> Result<Option<$ne>, <D as Fallible>::Error> {
                Ok(field.as_ref().copied().map(<$nz>::from).map(<$nz>::get))
            }
        }
    };
}

impl_as_non_zero!(NonZeroU8, u8);
impl_as_non_zero!(NonZeroU16, u16);
impl_as_non_zero!(NonZeroU32, u32);
impl_as_non_zero!(NonZeroU64, u64);
