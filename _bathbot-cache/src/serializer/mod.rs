use std::{alloc::Layout, ptr::NonNull};

#[cfg(debug_assertions)]
use rkyv::ser::serializers::{CompositeSerializer, ScratchTracker};
#[cfg(not(debug_assertions))]
use rkyv::{ser::serializers::AllocSerializer, Serialize};
use rkyv::{
    ser::{
        serializers::{
            AlignedSerializer, AllocScratch, FallbackScratch, HeapScratch, SharedSerializeMap,
        },
        ScratchSpace, Serializer,
    },
    AlignedVec, Archive, ArchiveUnsized, Fallible,
};

pub(crate) use self::{
    multi::{MemberSerializer, MultiSerializer},
    single::SingleSerializer,
};

mod multi;
mod single;

const CHANNEL_SCRATCH_SIZE: usize = 0;
const CURRENT_USER_SCRATCH_SIZE: usize = 0;
const GUILD_SCRATCH_SIZE: usize = 0;
const MEMBER_SCRATCH_SIZE: usize = 0;
const ROLE_SCRATCH_SIZE: usize = 0;
const USER_SCRATCH_SIZE: usize = 0;

#[cfg(debug_assertions)]
type TrackedSerializer<const N: usize> = CompositeSerializer<
    AlignedSerializer<AlignedVec>,
    ScratchTracker<FallbackScratch<HeapScratch<N>, AllocScratch>>,
    SharedSerializeMap,
>;

pub struct CacheSerializer<const N: usize> {
    #[cfg(debug_assertions)]
    inner: TrackedSerializer<N>,
    #[cfg(not(debug_assertions))]
    inner: AllocSerializer<N>,
}

#[cfg(debug_assertions)]
impl<const N: usize> CacheSerializer<N> {
    fn into_components(
        self,
    ) -> (
        AlignedSerializer<AlignedVec>,
        ScratchTracker<FallbackScratch<HeapScratch<N>, AllocScratch>>,
        SharedSerializeMap,
    ) {
        self.inner.into_components()
    }
}

#[cfg(not(debug_assertions))]
impl<const N: usize> CacheSerializer<N> {
    fn new(
        scratch: FallbackScratch<HeapScratch<N>, AllocScratch>,
        shared: SharedSerializeMap,
    ) -> Self {
        Self {
            inner: AllocSerializer::new(Default::default(), scratch, shared),
        }
    }

    fn into_components(
        self,
    ) -> (
        AlignedSerializer<AlignedVec>,
        FallbackScratch<HeapScratch<N>, AllocScratch>,
        SharedSerializeMap,
    ) {
        self.inner.into_components()
    }

    fn into_bytes(self) -> AlignedVec {
        let (serializer, ..) = self.into_components();

        serializer.into_inner()
    }

    fn serialize<T: Serialize<Self>>(value: &T) -> Result<AlignedVec, <Self as Fallible>::Error> {
        let mut serializer = Self::default();
        serializer.serialize_value(value)?;

        Ok(serializer.into_bytes())
    }
}

impl<const N: usize> Default for CacheSerializer<N> {
    fn default() -> Self {
        Self {
            #[cfg(debug_assertions)]
            inner: TrackedSerializer::new(
                Default::default(),
                ScratchTracker::new(Default::default()),
                Default::default(),
            ),
            #[cfg(not(debug_assertions))]
            inner: Default::default(),
        }
    }
}

impl<const N: usize> Fallible for CacheSerializer<N> {
    #[cfg(debug_assertions)]
    type Error = <TrackedSerializer<N> as Fallible>::Error;
    #[cfg(not(debug_assertions))]
    type Error = <AllocSerializer<N> as Fallible>::Error;
}

impl<const N: usize> Serializer for CacheSerializer<N> {
    #[inline]
    fn pos(&self) -> usize {
        self.inner.pos()
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.inner.write(bytes)
    }

    #[inline]
    fn pad(&mut self, padding: usize) -> Result<(), Self::Error> {
        self.inner.pad(padding)
    }

    #[inline]
    fn align(&mut self, align: usize) -> Result<usize, Self::Error> {
        self.inner.align(align)
    }

    #[inline]
    fn align_for<T>(&mut self) -> Result<usize, Self::Error> {
        self.inner.align_for::<T>()
    }

    #[inline]
    unsafe fn resolve_aligned<T: Archive + ?Sized>(
        &mut self,
        value: &T,
        resolver: T::Resolver,
    ) -> Result<usize, Self::Error> {
        self.inner.resolve_aligned::<T>(value, resolver)
    }

    #[inline]
    unsafe fn resolve_unsized_aligned<T: ArchiveUnsized + ?Sized>(
        &mut self,
        value: &T,
        to: usize,
        metadata_resolver: T::MetadataResolver,
    ) -> Result<usize, Self::Error> {
        self.inner
            .resolve_unsized_aligned(value, to, metadata_resolver)
    }
}

impl<const N: usize> ScratchSpace for CacheSerializer<N> {
    #[inline]
    unsafe fn push_scratch(&mut self, layout: Layout) -> Result<NonNull<[u8]>, Self::Error> {
        self.inner.push_scratch(layout)
    }

    #[inline]
    unsafe fn pop_scratch(&mut self, ptr: NonNull<u8>, layout: Layout) -> Result<(), Self::Error> {
        self.inner.pop_scratch(ptr, layout)
    }
}
