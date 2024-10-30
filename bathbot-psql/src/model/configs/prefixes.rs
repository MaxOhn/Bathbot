use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    mem,
    ops::{Deref, DerefMut},
    ptr, slice,
};

use arrayvec::{ArrayVec, CapacityError};
use compact_str::CompactString;
use rkyv::{
    bytecheck::{CheckBytes, Verify},
    munge::munge,
    rancor::{Fallible, Panic, ResultExt, Source},
    rel_ptr::RelPtr,
    ser::{Allocator, Writer},
    string::{ArchivedString, StringResolver},
    validation::{ArchiveContext, ArchiveContextExt},
    with::{ArchiveWith, DeserializeWith, SerializeWith, With},
    Archive, Deserialize, Place, Portable, Serialize, SerializeUnsized,
};

pub type Prefix = CompactString;

pub const DEFAULT_PREFIX: &str = "<";

const PREFIXES_LEN: usize = Prefixes::LEN;
type PrefixesInner = ArrayVec<Prefix, PREFIXES_LEN>;

#[derive(Clone)]
pub struct Prefixes {
    inner: PrefixesInner,
}

impl Prefixes {
    pub const LEN: usize = 5;

    #[inline]
    pub fn remaining_capacity(&self) -> usize {
        self.inner.remaining_capacity()
    }

    #[inline]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&mut Prefix) -> bool,
    {
        self.inner.retain(f)
    }

    #[inline]
    pub fn dedup(&mut self) {
        for i in (1..self.inner.len()).rev() {
            if self.inner[i - 1] == self.inner[i] {
                self.inner.remove(i);
            }
        }
    }

    /// Fails if there are already [`PREFIXES_LEN`] many prefixes contained
    #[inline]
    pub fn try_push(&mut self, prefix: Prefix) -> Result<(), CapacityError<Prefix>> {
        self.inner.try_push(prefix)
    }

    pub(crate) fn deserialize(bytes: &[u8]) -> Self {
        let archived_prefixes = rkyv::access::<ArchivedPrefixes, Panic>(bytes).always_ok();

        rkyv::api::deserialize_using::<_, _, Panic>(archived_prefixes, &mut ()).always_ok()
    }
}

impl Default for Prefixes {
    #[inline]
    fn default() -> Self {
        let mut inner = PrefixesInner::new();

        // SAFETY: The length is guaranteed to be 0 which is less than `PREFIXES_LEN`
        unsafe { inner.push_unchecked(DEFAULT_PREFIX.into()) };

        Self { inner }
    }
}

impl Deref for Prefixes {
    type Target = [Prefix];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Prefixes {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Extend<Prefix> for Prefixes {
    #[inline]
    fn extend<T: IntoIterator<Item = Prefix>>(&mut self, iter: T) {
        self.inner.extend(iter)
    }
}

#[derive(CheckBytes, Portable)]
#[bytecheck(crate = rkyv::bytecheck, verify)]
#[repr(C)]
pub struct ArchivedPrefixes {
    ptr: RelPtr<<PrefixRkyv as ArchiveWith<Prefix>>::Archived, i8>,
    len: u8,
}

impl ArchivedPrefixes {
    fn as_ptr(&self) -> *const <PrefixRkyv as ArchiveWith<Prefix>>::Archived {
        unsafe { self.ptr.as_ptr() }
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    fn as_slice(&self) -> &[<PrefixRkyv as ArchiveWith<Prefix>>::Archived] {
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len()) }
    }
}

unsafe impl<C> Verify<C> for ArchivedPrefixes
where
    ArchivedString: CheckBytes<C>,
    C: Fallible<Error: Source> + ArchiveContext + ?Sized,
{
    fn verify(&self, context: &mut C) -> Result<(), <C as Fallible>::Error> {
        let ptr = ptr::slice_from_raw_parts(self.ptr.as_ptr_wrapping(), self.len as usize);

        context.in_subtree(ptr, |context| unsafe {
            <[ArchivedString]>::check_bytes(ptr, context)
        })
    }
}

pub struct PrefixesResolver {
    pos: u8,
}

impl Archive for Prefixes {
    type Archived = ArchivedPrefixes;
    type Resolver = PrefixesResolver;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedPrefixes { ptr, len: out_len } = out);
        RelPtr::emplace(resolver.pos as usize, ptr);
        u8::resolve(&(self.inner.len() as u8), (), out_len);
    }
}

impl<S: Fallible<Error: Source> + Allocator + Writer + ?Sized> Serialize<S> for Prefixes {
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, <S as Fallible>::Error> {
        fn interprete_as_with(prefixes: &[Prefix]) -> &[With<Prefix, PrefixRkyv>] {
            // SAFETY: With<T, _> is just T under the hood
            unsafe { mem::transmute(prefixes) }
        }

        interprete_as_with(self.inner.as_slice())
            .serialize_unsized(serializer)
            .map(|pos| PrefixesResolver { pos: pos as u8 })
    }
}

impl<D: Fallible + ?Sized> Deserialize<Prefixes, D> for ArchivedPrefixes {
    fn deserialize(&self, deserializer: &mut D) -> Result<Prefixes, <D as Fallible>::Error> {
        let inner = self
            .as_slice()
            .iter()
            .map(|item| With::<_, PrefixRkyv>::cast(item).deserialize(deserializer))
            .collect::<Result<_, _>>()?;

        Ok(Prefixes { inner })
    }
}

impl Debug for Prefixes {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <PrefixesInner as Debug>::fmt(&self.inner, f)
    }
}

struct PrefixRkyv;

impl ArchiveWith<Prefix> for PrefixRkyv {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    fn resolve_with(field: &Prefix, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedString::resolve_from_str(field.as_str(), resolver, out);
    }
}

impl<S: Fallible<Error: Source> + Writer + ?Sized> SerializeWith<Prefix, S> for PrefixRkyv {
    fn serialize_with(field: &Prefix, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(field.as_str(), serializer)
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<ArchivedString, Prefix, D> for PrefixRkyv {
    fn deserialize_with(field: &ArchivedString, _: &mut D) -> Result<Prefix, D::Error> {
        Ok(Prefix::from(field.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dedup() {
        let inner = PrefixesInner::default();
        let mut prefixes = Prefixes { inner };

        prefixes.dedup();

        assert!(prefixes.is_empty());
    }

    #[test]
    fn noop_dedup() {
        let mut prefixes = Prefixes::default();
        prefixes.try_push("a".into()).unwrap();
        prefixes.try_push("b".into()).unwrap();
        prefixes.try_push("a".into()).unwrap();

        let orig = prefixes.clone();
        prefixes.dedup();

        assert_eq!(prefixes.inner, orig.inner);
    }

    #[test]
    fn do_dedup() {
        let mut prefixes = Prefixes::default();
        prefixes.try_push("a".into()).unwrap();
        prefixes.try_push("b".into()).unwrap();
        prefixes.try_push("b".into()).unwrap();
        prefixes.try_push("b".into()).unwrap();

        prefixes.dedup();

        let mut expected = Prefixes::default();
        expected.try_push("a".into()).unwrap();
        expected.try_push("b".into()).unwrap();

        assert_eq!(prefixes.inner, expected.inner);
    }
}
