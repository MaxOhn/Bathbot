pub struct FlagsRkyv;

#[macro_export]
macro_rules! with_flags_rkyv {
    ( $( $flags:ident ),* ) => {
        $(
            impl ::rkyv::with::ArchiveWith<$flags> for $crate::rkyv_util::FlagsRkyv {
                type Archived = $flags;
                type Resolver = ();

                #[inline]
                unsafe fn resolve_with(flags: &$flags, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
                    ::rkyv::Archive::resolve(&flags.bits(), pos, resolver, out.cast());
                }
            }

            impl<S: ::rkyv::Fallible + ?Sized> ::rkyv::with::SerializeWith<$flags, S> for $crate::rkyv_util::FlagsRkyv {
                #[inline]
                fn serialize_with(_: &$flags, _: &mut S) -> Result<Self::Resolver, S::Error> {
                    Ok(())
                }
            }

            impl<D: ::rkyv::Fallible + ?Sized> ::rkyv::with::DeserializeWith<$flags, $flags, D> for $crate::rkyv_util::FlagsRkyv {
                #[inline]
                fn deserialize_with(flags: &$flags, _: &mut D) -> Result<$flags, D::Error> {
                    Ok(*flags)
                }
            }
        )*
    };
}
