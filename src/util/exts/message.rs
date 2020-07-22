use crate::{BotResult, Context};

use async_trait::async_trait;
use std::fmt::Display;
use tokio::time::{timeout, Duration};
use twilight::http::request::channel::message::create_message::{
    CreateMessage, CreateMessageError,
};
use twilight::model::{
    channel::{Message, ReactionType},
    gateway::payload::ReactionAdd,
    id::UserId,
};
use twilight_mention::Mention;

// TODO: Check permissions

#[async_trait]
pub trait MessageExt {
    /// Response with content, embed, attachment, ...
    ///
    /// Includes reaction_delete
    async fn build_response<'a, F>(&self, ctx: &'a Context, f: F) -> BotResult<()>
    where
        F: Send + FnOnce(CreateMessage<'a>) -> Result<CreateMessage<'a>, CreateMessageError>;

    /// Response with simple content
    ///
    /// Includes reaction_delete
    async fn respond<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()>;

    /// Response with simple content by tagging the author
    ///
    /// Includes reaction_delete
    async fn reply<C: Display + Send>(&self, ctx: &Context, content: C) -> BotResult<()>;

    /// Give the author 60s to delete the message by reacting with `❌`
    fn reaction_delete(&self, ctx: &Context, owner: UserId);
}

#[async_trait]
impl MessageExt for Message {
    async fn build_response<'a, F>(&self, ctx: &'a Context, f: F) -> BotResult<()>
    where
        F: Send + FnOnce(CreateMessage<'a>) -> Result<CreateMessage<'a>, CreateMessageError>,
    {
        f(ctx.http.create_message(self.channel_id))?
            .await?
            .reaction_delete(ctx, self.author.id);
        Ok(())
    }

    async fn respond<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        ctx.http
            .create_message(self.channel_id)
            .content(content)?
            .await?
            .reaction_delete(ctx, self.author.id);
        Ok(())
    }

    async fn reply<C: Display + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let content = format!("{}: {}", self.author.mention(), content);
        ctx.http
            .create_message(self.channel_id)
            .content(content)?
            .await?
            .reaction_delete(ctx, self.author.id);
        Ok(())
    }

    fn reaction_delete(&self, ctx: &Context, owner: UserId) {
        assert_eq!(self.author.id, ctx.cache.bot_user.id);
        let standby = ctx.standby.clone();
        let http = ctx.http.clone();
        let channel_id = self.channel_id;
        let msg_id = self.id;
        tokio::spawn(async move {
            let reaction_result = timeout(
                Duration::from_secs(60),
                standby.wait_for_reaction(msg_id, move |event: &ReactionAdd| {
                    if event.user_id != owner {
                        return false;
                    }
                    if let ReactionType::Unicode { ref name } = event.0.emoji {
                        return name == "❌";
                    }
                    false
                }),
            )
            .await;
            if let Ok(Ok(_)) = reaction_result {
                if let Err(why) = http.delete_message(channel_id, msg_id).await {
                    warn!("Error while reaction-deleting msg: {}", why);
                }
            }
        });
    }
}
