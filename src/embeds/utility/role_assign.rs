use crate::embeds::EmbedData;

use serenity::{
    cache::Cache,
    model::{
        channel::Message,
        id::{GuildId, RoleId},
        misc::Mentionable,
    },
    utils::{content_safe, ContentSafeOptions},
};

#[derive(Clone)]
pub struct RoleAssignEmbed {
    description: String,
}

impl RoleAssignEmbed {
    pub async fn new(msg: Message, guild: GuildId, role: RoleId, cache: &Cache) -> Self {
        let description = format!(
            "Whoever reacts to {author}'s [message]\
            (https://discordapp.com/channels/{guild}/{channel}/{msg})\n\
            ```\n{content}\n```\n\
            in {channel_mention} will be assigned the {role_mention} role!",
            author = msg.author.mention(),
            guild = guild,
            channel = msg.channel_id,
            msg = msg.id,
            content = content_safe(cache, &msg.content, &ContentSafeOptions::default()).await,
            channel_mention = msg.channel_id.mention(),
            role_mention = role.mention(),
        );
        Self { description }
    }
}

impl EmbedData for RoleAssignEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
}
