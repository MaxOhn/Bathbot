use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    ops::{Deref, DerefMut},
    ptr, slice,
};

use arrayvec::{ArrayVec, CapacityError};
use compact_str::CompactString;
use rkyv::{
    from_archived, out_field,
    rel_ptr::signed_offset,
    ser::{ScratchSpace, Serializer},
    Archive, Archived, Deserialize, Fallible, Infallible, Serialize, SerializeUnsized,
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

    /// # Safety
    ///
    /// The caller must ensure that the provided bytes are valid archived
    /// prefixes
    pub(crate) unsafe fn deserialize(bytes: &[u8]) -> Self {
        let archived_prefixes = rkyv::archived_root::<Self>(bytes);

        archived_prefixes.deserialize(&mut Infallible).unwrap()
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

pub struct ArchivedPrefixes {
    ptr: RelPrefixesPtr,
    len: Archived<u8>,
}

impl ArchivedPrefixes {
    fn as_ptr(&self) -> *const <Prefix as Archive>::Archived {
        self.ptr.as_ptr()
    }

    pub fn len(&self) -> usize {
        from_archived!(self.len) as usize
    }

    fn as_slice(&self) -> &[<Prefix as Archive>::Archived] {
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len()) }
    }
}

pub struct PrefixesResolver {
    pos: u8,
}

impl Archive for Prefixes {
    type Archived = ArchivedPrefixes;
    type Resolver = PrefixesResolver;

    #[inline]
    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        // The prefixes were already serialized, next:

        // Serialize the relative ptr i.e. the offset to the start of the serialized
        // prefixes
        let (fp, fo) = out_field!(out.ptr);
        RelPrefixesPtr::emplace(pos + fp, resolver.pos as usize, fo);

        // Then serialize the length of the serialized prefixes
        let (fp, fo) = out_field!(out.len);
        let len = self.inner.len() as u8;
        <u8 as Archive>::resolve(&len, pos + fp, (), fo);
    }
}

impl<S: Serializer + ScratchSpace + ?Sized> Serialize<S> for Prefixes {
    #[inline]
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, <S as Fallible>::Error> {
        self.inner
            .serialize_unsized(serializer)
            .map(|pos| PrefixesResolver { pos: pos as u8 })
    }
}

impl<D: Fallible> Deserialize<Prefixes, D> for <Prefixes as Archive>::Archived {
    #[inline]
    fn deserialize(&self, deserializer: &mut D) -> Result<Prefixes, <D as Fallible>::Error> {
        let inner = self
            .as_slice()
            .iter()
            .map(|item| item.deserialize(deserializer))
            .collect::<Result<_, _>>()?;

        Ok(Prefixes { inner })
    }
}

/// Same as [`rkyv::RelPtr`] but specialized for prefixes
/// so its size is reduced and generics removed.
#[derive(Debug)]
struct RelPrefixesPtr {
    offset: i8,
}

impl RelPrefixesPtr {
    fn base(&self) -> *const u8 {
        (self as *const Self).cast::<u8>()
    }

    fn offset(&self) -> isize {
        self.offset as isize
    }

    fn as_ptr(&self) -> *const <Prefix as Archive>::Archived {
        unsafe { self.base().offset(self.offset()).cast() }
    }

    pub unsafe fn emplace(from: usize, to: usize, out: *mut Self) {
        let offset = signed_offset(from, to).unwrap();
        ptr::addr_of_mut!((*out).offset).write(offset as i8);
    }
}

impl Debug for Prefixes {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <PrefixesInner as Debug>::fmt(&self.inner, f)
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
