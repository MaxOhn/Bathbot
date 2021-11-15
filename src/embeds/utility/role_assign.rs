use twilight_model::{
    channel::Message,
    id::{GuildId, RoleId},
};

pub struct RoleAssignEmbed {
    description: String,
}

impl RoleAssignEmbed {
    pub async fn new(msg: Message, guild: GuildId, role: RoleId) -> Self {
        let content = msg.content.clone();

        let description = format!(
            "Whoever reacts to <@{author}>'s [message]\
            (https://discordapp.com/channels/{guild}/{channel}/{msg})\n\
            ```\n{content}\n```\n\
            in <#{channel_mention}> will be assigned the <@&{role_mention}> role!",
            author = msg.author.id,
            guild = guild,
            channel = msg.channel_id,
            msg = msg.id,
            content = content,
            channel_mention = msg.channel_id,
            role_mention = role,
        );

        Self { description }
    }
}

impl_builder!(RoleAssignEmbed { description });
