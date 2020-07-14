use crate::{
    util::{constants::CONTENT_LENGTH, error::CreateMessageError},
    BotResult, Context, Error,
};

use async_trait::async_trait;
use twilight::{http::request::channel::message::CreateMessage, model::channel::Message};
use twilight_mention::Mention;

#[async_trait]
pub trait MessageExt {
    async fn send_message<'a, F>(&self, ctx: &Context, f: F) -> BotResult<Message>
    where
        for<'b> F: Send + FnOnce(&'b mut CreateMessage<'a>) -> &'b mut CreateMessage<'a>;
    async fn say(&self, ctx: &Context, content: String) -> BotResult<Message>;
    async fn reply(&self, ctx: &Context, content: String) -> BotResult<Message>;
}

#[async_trait]
impl MessageExt for Message {
    async fn send_message<'a, F>(&self, ctx: &Context, f: F) -> BotResult<Message>
    where
        for<'b> F: Send + FnOnce(&'b mut CreateMessage<'a>) -> &'b mut CreateMessage<'a>,
    {
        Err(Error::NoConfig)
    }
    async fn say(&self, ctx: &Context, content: String) -> BotResult<Message> {
        Err(Error::NoConfig)
    }
    async fn reply(&self, ctx: &Context, content: String) -> BotResult<Message> {
        // {
        //     if self.guild_id.is_some() {
        //         let req = Permissions::SEND_MESSAGES;
        //         if !super::utils::user_has_perms(cache, self.channel_id, self.guild_id, req).await? {
        //             return Err(Error::Model(ModelError::InvalidPermissions(req)));
        //         }
        //     }
        // }

        // TODO: Check permissions
        if content.chars().count() > CONTENT_LENGTH as usize {
            return Err(CreateMessageError::ContentSize(content.chars().count()).into());
        }
        let content = format!("{}: {}", self.author.mention(), content);
        let response = ctx
            .http
            .create_message(self.channel_id)
            .content(content)?
            .await?;
        Ok(response)
    }
}
