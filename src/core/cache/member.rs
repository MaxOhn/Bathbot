use super::is_default;
use crate::core::cache::{Cache, CachedUser};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use twilight_model::gateway::payload::MemberUpdate;
use twilight_model::guild::Member;
use twilight_model::id::{RoleId, UserId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CachedMember {
    #[serde(rename = "i", default, skip_serializing_if = "is_default")]
    pub user_id: UserId,
    #[serde(rename = "n", default, skip_serializing_if = "is_default")]
    pub nickname: Option<String>,
    #[serde(rename = "r", default, skip_serializing_if = "is_default")]
    pub roles: Vec<RoleId>,
    #[serde(rename = "j", default, skip_serializing_if = "is_default")]
    pub joined_at: Option<String>,
}

impl CachedMember {
    pub fn from_member(member: &Member) -> Self {
        CachedMember {
            user_id: member.user.id,
            nickname: member.nick.clone(),
            roles: member.roles.clone(),
            joined_at: member.joined_at.clone(),
        }
    }

    pub fn update(&self, member: &MemberUpdate) -> Self {
        CachedMember {
            user_id: member.user.id,
            nickname: member.nick.clone(),
            roles: member.roles.clone(),
            joined_at: self.joined_at.clone(),
        }
    }

    pub fn user(&self, cache: &Cache) -> Option<Arc<CachedUser>> {
        cache
            .users
            .get(&self.user_id)
            .map(|guard| guard.value().clone())
    }

    pub fn duplicate(&self) -> Self {
        CachedMember {
            user_id: self.user_id,
            nickname: self.nickname.clone(),
            roles: self.roles.clone(),
            joined_at: self.joined_at.clone(),
        }
    }
}
