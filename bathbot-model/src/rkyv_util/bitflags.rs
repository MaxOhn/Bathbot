pub struct FlagsRkyv;

#[macro_export]
macro_rules! with_flags_rkyv {
    ( $( $flags:ident: $int:ty ),* ) => {
        $(
            impl ::rkyv::with::ArchiveWith<$flags> for $crate::rkyv_util::FlagsRkyv {
                type Archived = ::rkyv::Archived<$int>;
                type Resolver = ();

                #[inline]
                fn resolve_with(
                    flags: &$flags,
                    resolver: Self::Resolver,
                    out: ::rkyv::Place<Self::Archived>,)
                {
                    ::rkyv::Archive::resolve(&flags.bits(), resolver, out);
                }
            }

            impl<S: ::rkyv::rancor::Fallible + ?Sized> ::rkyv::with::SerializeWith<$flags, S>
                for $crate::rkyv_util::FlagsRkyv
            {
                #[inline]
                fn serialize_with(_: &$flags, _: &mut S) -> Result<Self::Resolver, S::Error> {
                    Ok(())
                }
            }

            impl<D: ::rkyv::rancor::Fallible + ?Sized>
                ::rkyv::with::DeserializeWith<
                    ::rkyv::Archived<$int>,
                    $flags,
                    D,
                > for $crate::rkyv_util::FlagsRkyv
            {
                #[inline]
                fn deserialize_with(flags: &::rkyv::Archived<$int>, _: &mut D)
                    -> Result<$flags, D::Error>
                {
                    Ok($flags::from_bits_truncated((*flags).into()))
                }
            }
        )*
    };
}
