use crate::{BotResult, Context};

use twilight::model::id::{ChannelId, MessageId, RoleId};

impl Context {
    pub fn get_link(&self, discord_id: u64) -> Option<String> {
        self.data
            .discord_links
            .get(&discord_id)
            .map(|guard| guard.value().to_owned())
    }

    pub async fn add_link(&self, discord_id: u64, osu_name: impl Into<String>) -> BotResult<()> {
        let name = osu_name.into();
        self.clients
            .psql
            .add_discord_link(discord_id, &name)
            .await?;
        self.data.discord_links.insert(discord_id, name);
        Ok(())
    }

    pub async fn remove_link(&self, discord_id: u64) -> BotResult<()> {
        self.clients.psql.remove_discord_link(discord_id).await?;
        self.data.discord_links.remove(&discord_id);
        Ok(())
    }

    pub fn add_role_assign(&self, channel_id: ChannelId, msg_id: MessageId, role_id: RoleId) {
        self.data
            .role_assigns
            .insert((channel_id.0, msg_id.0), role_id.0);
    }
}
