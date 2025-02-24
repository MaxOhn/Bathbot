mod map;

use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    marker::PhantomData,
    num::NonZeroU64,
};

use rkyv::{
    Archive, Archived, Place, Portable,
    bytecheck::CheckBytes,
    munge::munge,
    rancor::Fallible,
    traits::NoUndef,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
};
use twilight_model::id::Id;

pub use self::map::{ArchivedIdOption, IdRkyvMap};

/// Used to archive [`Id<T>`].
///
/// # Example
///
/// ```
/// # use rkyv::Archive;
/// use redlight::rkyv_util::id::IdRkyv;
/// use twilight_model::id::Id;
///
/// #[derive(Archive)]
/// struct Cached<T> {
///     #[rkyv(with = IdRkyv)]
///     id: Id<T>,
/// }
/// ```
pub struct IdRkyv;

#[derive(Portable, CheckBytes)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(C)]
pub struct ArchivedId<T> {
    value: Archived<NonZeroU64>,
    _phantom: PhantomData<fn(T) -> T>,
}

impl<T> ArchiveWith<Id<T>> for IdRkyv {
    type Archived = ArchivedId<T>;
    type Resolver = ();

    fn resolve_with(field: &Id<T>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedId { value, _phantom } = out);
        field.into_nonzero().resolve(resolver, value);
    }
}

impl<T, S: Fallible + ?Sized> SerializeWith<Id<T>, S> for IdRkyv {
    fn serialize_with(_: &Id<T>, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<T, D> DeserializeWith<ArchivedId<T>, Id<T>, D> for IdRkyv
where
    D: Fallible + ?Sized,
{
    fn deserialize_with(
        archived: &ArchivedId<T>,
        _: &mut D,
    ) -> Result<Id<T>, <D as Fallible>::Error> {
        Ok(Id::from(*archived))
    }
}

impl<T> ArchivedId<T> {
    pub const fn to_native(self) -> Id<T> {
        unsafe { Id::new_unchecked(self.value.get()) }
    }

    /// Return the inner primitive value.
    pub fn get(self) -> u64 {
        self.into_nonzero().get()
    }

    /// Return the [`NonZeroU64`] representation of the ID.
    pub fn into_nonzero(self) -> NonZeroU64 {
        self.value.into()
    }

    /// Cast an archived ID from one type to another.
    pub const fn cast<New>(self) -> ArchivedId<New> {
        ArchivedId {
            value: self.value,
            _phantom: PhantomData,
        }
    }
}

impl<T> Clone for ArchivedId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ArchivedId<T> {}

impl<T> Display for ArchivedId<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.value, f)
    }
}

impl<T> Debug for ArchivedId<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}

impl<T> From<Id<T>> for ArchivedId<T> {
    fn from(value: Id<T>) -> Self {
        Self {
            value: value.into_nonzero().into(),
            _phantom: PhantomData,
        }
    }
}

impl<T> From<ArchivedId<T>> for Id<T> {
    fn from(id: ArchivedId<T>) -> Self {
        id.to_native()
    }
}

impl<T> PartialEq for ArchivedId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for ArchivedId<T> {}

impl<T> PartialEq<Id<T>> for ArchivedId<T> {
    fn eq(&self, other: &Id<T>) -> bool {
        self.value == other.into_nonzero()
    }
}

impl<T> PartialEq<ArchivedId<T>> for Id<T> {
    fn eq(&self, other: &ArchivedId<T>) -> bool {
        other.eq(self)
    }
}

unsafe impl<T> NoUndef for ArchivedId<T> {}
