use super::{Cache, CachedChannel, CachedGuild, CachedMember, CachedRole, CachedUser};

use std::sync::Arc;
use twilight::model::{
    id::{ChannelId, GuildId, RoleId, UserId},
    user::User,
};

impl Cache {
    pub fn has_admin_permission(&self, user_id: UserId, guild_id: GuildId) -> Option<bool> {
        self.guilds
            .get(&guild_id)
            .and_then(|guard| guard.value().has_admin_permission(user_id))
    }

    pub fn is_guild_owner(&self, guild_id: GuildId, user_id: UserId) -> bool {
        self.guilds
            .get(&guild_id)
            .map(|guard| guard.value().owner_id == user_id)
            .unwrap_or(false)
    }

    pub fn get_guild(&self, guild_id: GuildId) -> Option<Arc<CachedGuild>> {
        self.guilds
            .get(&guild_id)
            .map(|guard| guard.value().clone())
    }

    pub fn get_user(&self, user_id: UserId) -> Option<Arc<CachedUser>> {
        self.users.get(&user_id).map(|guard| guard.value().clone())
    }

    pub fn get_or_insert_user(&self, user: &User) -> Arc<CachedUser> {
        match self.get_user(user.id) {
            Some(user) => user,
            None => {
                let arc = Arc::new(CachedUser::from_user(user));
                self.users.insert(arc.id, arc.clone());
                self.stats.user_counts.unique.inc();
                arc
            }
        }
    }
    pub fn get_role(&self, role_id: RoleId, guild_id: GuildId) -> Option<Arc<CachedRole>> {
        self.get_guild(guild_id)
            .and_then(|guild| Some(guild.roles.get(&role_id)?.value().clone()))
    }

    pub fn get_guild_channel(
        &self,
        channel_id: ChannelId,
        guild_id: GuildId,
    ) -> Option<Arc<CachedChannel>> {
        self.get_guild(guild_id)
            .and_then(|guild| Some(guild.channels.get(&channel_id)?.value().clone()))
    }

    pub fn get_member(&self, user_id: UserId, guild_id: GuildId) -> Option<Arc<CachedMember>> {
        self.get_guild(guild_id)
            .and_then(|guild| Some(guild.members.get(&user_id)?.value().clone()))
    }
}
