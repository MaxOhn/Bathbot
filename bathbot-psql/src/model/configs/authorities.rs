use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    ops::{Deref, DerefMut},
};

use rkyv::{
    ser::{ScratchSpace, Serializer},
    vec::{ArchivedVec, VecResolver},
    Archive, Deserialize, Fallible, Infallible, Serialize,
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
    type Archived = ArchivedVec<<Authority as Archive>::Archived>;
    type Resolver = VecResolver;

    #[inline]
    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        ArchivedVec::resolve_from_slice(&self.inner, pos, resolver, out)
    }
}

impl<S: Serializer + ScratchSpace + ?Sized> Serialize<S> for Authorities {
    #[inline]
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, <S as Fallible>::Error> {
        ArchivedVec::serialize_from_slice(&self.inner, serializer)
    }
}

impl<D: Fallible> Deserialize<Authorities, D> for <Authorities as Archive>::Archived {
    #[inline]
    fn deserialize(&self, deserializer: &mut D) -> Result<Authorities, <D as Fallible>::Error> {
        let inner = self
            .as_slice()
            .iter()
            .map(|item| item.deserialize(deserializer))
            .collect::<Result<_, _>>()?;

        Ok(Authorities { inner })
    }
}

impl Debug for Authorities {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <AuthoritiesInner as Debug>::fmt(&self.inner, f)
    }
}
