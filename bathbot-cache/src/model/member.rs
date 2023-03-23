use rkyv::{
    with::{Inline, Map, RefAsBox},
    Archive, Serialize,
};
use twilight_model::{
    application::interaction::application_command::InteractionMember,
    gateway::payload::incoming::MemberUpdate,
    guild::{Member, PartialMember},
    id::{marker::RoleMarker, Id},
    util::ImageHash,
};

#[derive(Archive, Serialize)]
pub struct CachedMember<'m> {
    #[with(Map<Inline>)]
    pub avatar: Option<&'m ImageHash>,
    #[with(Map<RefAsBox>)]
    pub nick: Option<&'m str>,
    #[with(RefAsBox)]
    pub roles: &'m [Id<RoleMarker>],
}

impl<'m> From<&'m Member> for CachedMember<'m> {
    #[inline]
    fn from(member: &'m Member) -> Self {
        Self {
            avatar: member.avatar.as_ref(),
            nick: member.nick.as_deref(),
            roles: member.roles.as_slice(),
        }
    }
}

impl<'m> From<&'m PartialMember> for CachedMember<'m> {
    #[inline]
    fn from(member: &'m PartialMember) -> Self {
        Self {
            avatar: member.avatar.as_ref(),
            nick: member.nick.as_deref(),
            roles: member.roles.as_slice(),
        }
    }
}

impl<'m> From<&'m InteractionMember> for CachedMember<'m> {
    #[inline]
    fn from(member: &'m InteractionMember) -> Self {
        Self {
            avatar: member.avatar.as_ref(),
            nick: member.nick.as_deref(),
            roles: member.roles.as_slice(),
        }
    }
}

impl<'m> From<&'m MemberUpdate> for CachedMember<'m> {
    #[inline]
    fn from(update: &'m MemberUpdate) -> Self {
        Self {
            avatar: update.avatar.as_ref(),
            nick: update.nick.as_deref(),
            roles: update.roles.as_slice(),
        }
    }
}
