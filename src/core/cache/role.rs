use super::is_default;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use twilight::model::{
    guild::{Permissions, Role},
    id::RoleId,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedRole {
    #[serde(rename = "a")]
    pub id: RoleId,
    #[serde(rename = "b")]
    pub name: String,
    #[serde(rename = "c", default, skip_serializing_if = "is_default")]
    pub color: u32,
    #[serde(rename = "d", default, skip_serializing_if = "is_default")]
    pub hoisted: bool,
    #[serde(rename = "e", default, skip_serializing_if = "is_default")]
    pub position: i64,
    #[serde(rename = "f")]
    pub permissions: Permissions,
    #[serde(rename = "g", default, skip_serializing_if = "is_default")]
    pub managed: bool,
    #[serde(rename = "h", default, skip_serializing_if = "is_default")]
    pub mentionable: bool,
}

impl CachedRole {
    pub fn from_role(role: &Role) -> Self {
        CachedRole {
            id: role.id,
            name: role.name.clone(),
            color: role.color,
            hoisted: role.hoist,
            position: role.position,
            permissions: role.permissions,
            managed: role.managed,
            mentionable: role.mentionable,
        }
    }
}

impl From<Arc<CachedRole>> for CachedRole {
    fn from(role: Arc<CachedRole>) -> Self {
        CachedRole {
            id: role.id,
            name: role.name.clone(),
            color: role.color,
            hoisted: role.hoisted,
            position: role.position,
            permissions: role.permissions,
            managed: role.managed,
            mentionable: role.mentionable,
        }
    }
}
