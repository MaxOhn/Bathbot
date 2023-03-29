use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Fallible,
};
use twilight_model::guild::Permissions;

pub struct FlagsRkyv;

macro_rules! with_flags_rkyv {
    ( $( $flags:ident $(,)? )* ) => {
        $(
            impl ArchiveWith<$flags> for FlagsRkyv {
                type Archived = $flags;
                type Resolver = ();

                #[inline]
                unsafe fn resolve_with(flags: &$flags, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
                    Archive::resolve(&flags.bits(), pos, resolver, out.cast());
                }
            }

            impl<S: Fallible + ?Sized> SerializeWith<$flags, S> for FlagsRkyv {
                #[inline]
                fn serialize_with(_: &$flags, _: &mut S) -> Result<Self::Resolver, S::Error> {
                    Ok(())
                }
            }

            impl<D: Fallible + ?Sized> DeserializeWith<u64, $flags, D> for FlagsRkyv {
                #[inline]
                fn deserialize_with(bits: &u64, _: &mut D) -> Result<$flags, D::Error> {
                    Ok($flags::from_bits_truncate(*bits))
                }
            }
        )*
    };
}

with_flags_rkyv!(Permissions);
