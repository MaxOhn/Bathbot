use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    mem,
    num::NonZeroU64,
    ops::{Deref, DerefMut},
};

use rkyv::{
    ser::{ScratchSpace, Serializer},
    vec::{ArchivedVec, VecResolver},
    Archive, Archived, Deserialize, Fallible, Infallible, Serialize,
};
use smallvec::SmallVec;
use twilight_model::id::{marker::RoleMarker, Id};

pub type Authority = Id<RoleMarker>;

type AuthoritiesInner = SmallVec<[Authority; 4]>;

#[derive(Clone, Default)]
pub struct Authorities {
    inner: AuthoritiesInner,
}

impl Authorities {
    pub fn push(&mut self, authority: Authority) {
        self.inner.push(authority)
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&mut Authority) -> bool,
    {
        self.inner.retain(f);
    }

    /// # Safety
    ///
    /// The caller must ensure that the provided bytes are valid archived authorities
    pub(crate) unsafe fn deserialize(bytes: &[u8]) -> Self {
        let archived_authorities = rkyv::archived_root::<Self>(bytes);

        archived_authorities.deserialize(&mut Infallible).unwrap()
    }
}

impl Deref for Authorities {
    type Target = [Authority];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Authorities {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl FromIterator<Authority> for Authorities {
    #[inline]
    fn from_iter<T: IntoIterator<Item = Authority>>(iter: T) -> Self {
        Self {
            inner: iter.into_iter().collect(),
        }
    }
}

impl Archive for Authorities {
    type Archived = Archived<Vec<NonZeroU64>>;
    type Resolver = VecResolver;

    #[inline]
    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        ArchivedVec::resolve_from_len(self.inner.len(), pos, resolver, out);
    }
}

impl<S: Serializer + ScratchSpace + ?Sized> Serialize<S> for Authorities {
    #[inline]
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, <S as Fallible>::Error> {
        fn interprete_as_u64s<T>(ids: &[Id<T>]) -> &[NonZeroU64] {
            // SAFETY: Id<T> is just NonZeroU64 under the hood
            unsafe { mem::transmute(ids) }
        }

        ArchivedVec::serialize_from_slice(interprete_as_u64s(&self.inner), serializer)
    }
}

impl<D: Fallible> Deserialize<Authorities, D> for <Authorities as Archive>::Archived {
    #[inline]
    fn deserialize(&self, _: &mut D) -> Result<Authorities, <D as Fallible>::Error> {
        fn interprete_as_ids<T>(ids: &[NonZeroU64]) -> &[Id<T>] {
            // SAFETY: Id<T> is just NonZeroU64 under the hood
            unsafe { mem::transmute(ids) }
        }

        let ids = interprete_as_ids(self.as_slice());
        let inner = AuthoritiesInner::from_slice(ids);

        Ok(Authorities { inner })
    }
}

impl Debug for Authorities {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <AuthoritiesInner as Debug>::fmt(&self.inner, f)
    }
}
