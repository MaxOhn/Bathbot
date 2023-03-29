use std::{collections::HashMap, hash::BuildHasher};

use rkyv::{
    collections::{hash_map::HashMapResolver, ArchivedHashMap},
    out_field,
    ser::{ScratchSpace, Serializer},
    with::{ArchiveWith, DeserializeWith, RefAsBox, SerializeWith, With},
    Archive, Archived, Deserialize, Fallible, Resolver, Serialize,
};
use twilight_gateway::Session;

pub struct SessionRkyv;

pub struct ArchivedSession {
    pub id: Archived<Box<str>>,
    pub sequence: Archived<u64>,
}

pub struct SessionResolver {
    pub id: Resolver<Box<str>>,
}

impl ArchiveWith<Session> for SessionRkyv {
    type Archived = ArchivedSession;
    type Resolver = SessionResolver;

    #[inline]
    unsafe fn resolve_with(
        session: &Session,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let (fp, fo) = out_field!(out.id);
        let id = session.id();
        let id = With::<_, RefAsBox>::cast(&id);
        Archive::resolve(id, pos + fp, resolver.id, fo);

        let (fp, fo) = out_field!(out.sequence);
        Archive::resolve(&session.sequence(), pos + fp, (), fo);
    }
}

impl<S: Fallible + Serializer + ?Sized> SerializeWith<Session, S> for SessionRkyv {
    #[inline]
    fn serialize_with(
        session: &Session,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(SessionResolver {
            id: Serialize::serialize(With::<_, RefAsBox>::cast(&session.id()), serializer)?,
        })
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<ArchivedSession, Session, D> for SessionRkyv {
    #[inline]
    fn deserialize_with(
        session: &ArchivedSession,
        deserializer: &mut D,
    ) -> Result<Session, <D as Fallible>::Error> {
        let id: Box<str> = session.id.deserialize(deserializer)?;
        let sequence = session.sequence;

        Ok(Session::new(sequence, id.into()))
    }
}

pub struct SessionsRkyv;

impl<S> ArchiveWith<HashMap<u64, Session, S>> for SessionsRkyv {
    type Archived = ArchivedHashMap<u64, Session>;
    type Resolver = HashMapResolver;

    #[inline]
    unsafe fn resolve_with(
        map: &HashMap<u64, Session, S>,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        ArchivedHashMap::resolve_from_len(map.len(), pos, resolver, out);
    }
}

impl<H, S: Fallible + ?Sized> SerializeWith<HashMap<u64, Session, H>, S> for SessionsRkyv
where
    S: Serializer + ScratchSpace,
{
    #[inline]
    fn serialize_with(
        map: &HashMap<u64, Session, H>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        let iter = map
            .iter()
            .map(|(shard_id, session)| (shard_id, With::<_, SessionRkyv>::cast(session)));

        unsafe { ArchivedHashMap::serialize_from_iter(iter, serializer) }
    }
}

impl<S, D>
    DeserializeWith<
        <Self as ArchiveWith<HashMap<u64, Session, S>>>::Archived,
        HashMap<u64, Session, S>,
        D,
    > for SessionsRkyv
where
    D: Fallible + ?Sized,
    S: BuildHasher + Default,
{
    #[inline]
    fn deserialize_with(
        map: &<Self as ArchiveWith<HashMap<u64, Session, S>>>::Archived,
        _: &mut D,
    ) -> Result<HashMap<u64, Session, S>, <D as Fallible>::Error> {
        let mut result = HashMap::with_capacity_and_hasher(map.len(), S::default());

        for (shard_id, session) in map.iter() {
            result.insert(*shard_id, session.to_owned());
        }

        Ok(result)
    }
}
