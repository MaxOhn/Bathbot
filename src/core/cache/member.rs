use super::is_default;
use crate::core::cache::{Cache, CachedUser};

use dashmap::ElementGuard;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use twilight_model::{
    gateway::payload::MemberUpdate,
    guild::Member,
    id::{RoleId, UserId},
};

#[derive(Debug, Clone)]
pub struct CachedMember {
    pub user: Arc<CachedUser>,
    pub nickname: Option<String>,
    pub roles: Vec<RoleId>,
    pub joined_at: Option<String>,
    //TODO: Convert to date
    pub boosting_since: Option<String>,
    pub server_deafened: bool,
    pub server_muted: bool,
}

impl CachedMember {
    pub fn defrost(member: ColdStorageMember, cache: &Cache) -> Self {
        CachedMember {
            user: cache.get_user(member.id).unwrap(),
            nickname: member.nickname,
            roles: member.roles,
            joined_at: member.joined_at,
            boosting_since: member.boosting_since,
            server_deafened: member.server_deafened,
            server_muted: member.server_muted,
        }
    }

    pub fn from_member(member: &Member, cache: &Cache) -> Self {
        CachedMember {
            user: cache.get_or_insert_user(&member.user),
            nickname: member.nick.clone(),
            roles: member.roles.clone(),
            joined_at: member.joined_at.clone(),
            boosting_since: member.premium_since.clone(),
            server_deafened: member.deaf,
            server_muted: member.mute,
        }
    }

    pub fn update(&self, member: &MemberUpdate, cache: &Cache) -> Self {
        CachedMember {
            user: cache.get_or_insert_user(&member.user),
            nickname: member.nick.clone(),
            roles: member.roles.clone(),
            joined_at: self.joined_at.clone(),
            boosting_since: member.premium_since.clone(),
            server_deafened: self.server_deafened,
            server_muted: self.server_muted,
        }
    }
}

impl CachedMember {
    pub fn replace_user(&self, user: Arc<CachedUser>) -> Self {
        CachedMember {
            user,
            nickname: self.nickname.clone(),
            roles: self.roles.clone(),
            joined_at: self.joined_at.clone(),
            boosting_since: self.boosting_since.clone(),
            server_deafened: self.server_deafened,
            server_muted: self.server_muted,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColdStorageMember {
    #[serde(rename = "i", default, skip_serializing_if = "is_default")]
    pub id: UserId,
    #[serde(rename = "n", default, skip_serializing_if = "is_default")]
    pub nickname: Option<String>,
    #[serde(rename = "r", default, skip_serializing_if = "is_default")]
    pub roles: Vec<RoleId>,
    #[serde(rename = "j", default, skip_serializing_if = "is_default")]
    pub joined_at: Option<String>,
    #[serde(rename = "b", default, skip_serializing_if = "is_default")]
    pub boosting_since: Option<String>,
    #[serde(rename = "d", default, skip_serializing_if = "is_default")]
    pub server_deafened: bool,
    #[serde(rename = "m", default, skip_serializing_if = "is_default")]
    pub server_muted: bool,
}

impl From<ElementGuard<UserId, Arc<CachedMember>>> for ColdStorageMember {
    fn from(member: ElementGuard<UserId, Arc<CachedMember>>) -> Self {
        ColdStorageMember {
            id: member.user.id,
            nickname: member.nickname.clone(),
            roles: member.roles.clone(),
            joined_at: member.joined_at.clone(),
            boosting_since: member.boosting_since.clone(),
            server_deafened: member.server_deafened,
            server_muted: member.server_muted,
        }
    }
}
