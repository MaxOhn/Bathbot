use crate::{util::content_safe, Context};

use twilight_model::{
    channel::Message,
    id::{GuildId, RoleId},
};

pub struct RoleAssignEmbed {
    description: String,
}

impl RoleAssignEmbed {
    pub async fn new(ctx: &Context, msg: Message, guild: GuildId, role: RoleId) -> Self {
        let mut content = msg.content.clone();
        content_safe(ctx, &mut content, Some(guild));

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
