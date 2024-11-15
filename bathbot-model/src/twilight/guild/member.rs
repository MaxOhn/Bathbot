use rkyv::{
    munge::munge,
    niche::option_box::ArchivedOptionBox,
    rancor::Fallible,
    ser::{Allocator, Writer},
    with::{ArchiveWith, Map, Niche, SerializeWith},
    Archive, Deserialize, Place, Serialize,
};
use twilight_model::{
    application::interaction::application_command::InteractionMember,
    gateway::payload::incoming::MemberUpdate,
    guild::{Member, PartialMember},
    id::{marker::RoleMarker, Id},
    util::ImageHash,
};

use crate::twilight::{id::IdRkyvMap, util::ImageHashRkyv};

#[derive(Archive, Serialize, Deserialize)]
pub struct CachedMember {
    #[rkyv(with = Map<ImageHashRkyv>)]
    pub avatar: Option<ImageHash>,
    #[rkyv(with = Niche)]
    pub nick: Option<Box<str>>,
    #[rkyv(with = IdRkyvMap)]
    pub roles: Box<[Id<RoleMarker>]>,
}

macro_rules! impl_with {
    ( $ty:ty ) => {
        impl ArchiveWith<$ty> for CachedMember {
            type Archived = ArchivedCachedMember;
            type Resolver = CachedMemberResolver;

            fn resolve_with(member: &$ty, resolver: Self::Resolver, out: Place<Self::Archived>) {
                munge!(let ArchivedCachedMember { avatar, nick, roles } = out);
                Map::<ImageHashRkyv>::resolve_with(&member.avatar, resolver.avatar, avatar);
                ArchivedOptionBox::resolve_from_option(member.nick.as_deref(), resolver.nick, nick);
                IdRkyvMap::resolve_with(&member.roles, resolver.roles, roles);
            }
        }

        impl<S: Fallible + Writer + Allocator + ?Sized> SerializeWith<$ty, S> for CachedMember {
            fn serialize_with(
                member: &$ty,
                serializer: &mut S,
            ) -> Result<Self::Resolver, S::Error> {
                Ok(CachedMemberResolver {
                    avatar: Map::<ImageHashRkyv>::serialize_with(&member.avatar, serializer)?,
                    nick: ArchivedOptionBox::serialize_from_option(
                        member.nick.as_deref(),
                        serializer,
                    )?,
                    roles: IdRkyvMap::serialize_with(&member.roles, serializer)?,
                })
            }
        }
    };
}

impl_with!(InteractionMember);
impl_with!(Member);
impl_with!(MemberUpdate);
impl_with!(PartialMember);
