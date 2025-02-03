use std::{collections::HashMap, hash::BuildHasher};

use rkyv::{
    bytecheck::CheckBytes,
    munge::munge,
    rancor::Fallible,
    ser::{Allocator, Writer},
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, DeserializeWith, InlineAsBox, SerializeWith},
    Archive, Archived, Deserialize, Place, Portable, Resolver, Serialize,
};
use twilight_gateway::Session;

type Sessions<H> = HashMap<u32, Session, H>;

pub struct SessionsRkyv;

pub type ArchivedSessions = ArchivedVec<ArchivedSessionEntry>;

impl<H> ArchiveWith<Sessions<H>> for SessionsRkyv {
    type Archived = ArchivedSessions;
    type Resolver = VecResolver;

    fn resolve_with(sessions: &Sessions<H>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(sessions.len(), resolver, out);
    }
}

impl<H, S> SerializeWith<Sessions<H>, S> for SessionsRkyv
where
    S: Fallible + Writer + Allocator + ?Sized,
{
    fn serialize_with(sessions: &Sessions<H>, s: &mut S) -> Result<Self::Resolver, S::Error> {
        let iter = sessions.iter().map(|(key, value)| SessionEntry {
            shard_id: *key,
            session_id: value.id(),
            session_sequence: value.sequence(),
        });

        ArchivedVec::serialize_from_iter(iter, s)
    }
}

impl<H, D> DeserializeWith<ArchivedSessions, HashMap<u32, Session, H>, D> for SessionsRkyv
where
    Archived<u32>: Deserialize<u32, D>,
    D: Fallible + ?Sized,
    H: Default + BuildHasher,
{
    fn deserialize_with(
        archived: &ArchivedSessions,
        _: &mut D,
    ) -> Result<HashMap<u32, Session, H>, D::Error> {
        let mut result = HashMap::with_capacity_and_hasher(archived.len(), H::default());

        for entry in archived.iter() {
            let shard_id = entry.shard_id.into();
            let session_id = entry.session_id.as_ref().to_owned();
            let session_sequence = entry.session_sequence.into();
            result.insert(shard_id, Session::new(session_sequence, session_id));
        }

        Ok(result)
    }
}

struct SessionEntry<'a> {
    shard_id: u32,
    session_id: &'a str,
    session_sequence: u64,
}

#[derive(Portable, CheckBytes)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(C)]
pub struct ArchivedSessionEntry {
    pub shard_id: Archived<u32>,
    pub session_id: Archived<Box<str>>,
    pub session_sequence: Archived<u64>,
}

struct SessionEntryResolver {
    shard_id: Resolver<u32>,
    session_id: Resolver<Box<str>>,
    session_sequence: Resolver<u64>,
}

impl Archive for SessionEntry<'_> {
    type Archived = ArchivedSessionEntry;
    type Resolver = SessionEntryResolver;

    #[allow(clippy::unit_arg)]
    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(
            let ArchivedSessionEntry {
                shard_id,
                session_id,
                session_sequence
            } = out
        );
        self.shard_id.resolve(resolver.shard_id, shard_id);
        InlineAsBox::resolve_with(&self.session_id, resolver.session_id, session_id);
        self.session_sequence
            .resolve(resolver.session_sequence, session_sequence);
    }
}

impl<S: Fallible + Writer + ?Sized> Serialize<S> for SessionEntry<'_> {
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(SessionEntryResolver {
            shard_id: self.shard_id.serialize(serializer)?,
            session_id: InlineAsBox::serialize_with(&self.session_id, serializer)?,
            session_sequence: self.session_sequence.serialize(serializer)?,
        })
    }
}
