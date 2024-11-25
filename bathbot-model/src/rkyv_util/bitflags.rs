use rkyv::{
    rancor::Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Place,
};
use twilight_model::{
    channel::{message::MessageFlags, ChannelFlags},
    gateway::presence::ActivityFlags,
    guild::{MemberFlags, Permissions, SystemChannelFlags},
    user::UserFlags,
};

/// Used to archive flag type such as [`Permissions`] or [`MemberFlags`].
///
/// # Example
///
/// ```
/// # use rkyv::Archive;
/// use redlight::rkyv_util::util::BitflagsRkyv;
/// use rkyv::with::Map;
/// use twilight_model::guild::{MemberFlags, Permissions};
///
/// #[derive(Archive)]
/// struct Cached {
///     #[rkyv(with = BitflagsRkyv)]
///     permissions: Permissions,
///     #[rkyv(with = Map<BitflagsRkyv>)]
///     member_flags: Option<MemberFlags>,
/// }
/// ```
pub struct BitflagsRkyv;

#[macro_export]
macro_rules! impl_bitflags {
    ($ty:ty: $ar:ty) => {
        impl ArchiveWith<$ty> for BitflagsRkyv {
            type Archived = Archived<$ar>;
            type Resolver = ();

            fn resolve_with(flags: &$ty, resolver: Self::Resolver, out: Place<Self::Archived>) {
                flags.bits().resolve(resolver, out);
            }
        }

        impl<S: Fallible + ?Sized> SerializeWith<$ty, S> for BitflagsRkyv {
            fn serialize_with(
                _: &$ty,
                _: &mut S,
            ) -> Result<Self::Resolver, <S as Fallible>::Error> {
                Ok(())
            }
        }

        impl<D: Fallible + ?Sized> DeserializeWith<Archived<u64>, $ty, D> for BitflagsRkyv {
            fn deserialize_with(
                archived: &Archived<u64>,
                _: &mut D,
            ) -> Result<$ty, <D as Fallible>::Error> {
                Ok(<$ty>::from_bits_truncate((*archived).into()))
            }
        }
    };
}

impl_bitflags!(ActivityFlags: u64);
impl_bitflags!(ChannelFlags: u64);
impl_bitflags!(MemberFlags: u64);
impl_bitflags!(MessageFlags: u64);
impl_bitflags!(Permissions: u64);
impl_bitflags!(SystemChannelFlags: u64);
impl_bitflags!(UserFlags: u64);
