use crate::{embeds::EmbedBuilder, util::constants::RED, BotResult, Context};

use async_trait::async_trait;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use twilight_http::request::channel::message::create_message::{CreateMessage, CreateMessageError};
use twilight_model::{
    channel::{embed::Embed, Message, ReactionType},
    gateway::payload::ReactionAdd,
    id::UserId,
};

#[async_trait]
pub trait MessageExt {
    /// Response with content, embed, attachment, ...
    async fn build_response_msg<'a, 'b, F>(&self, ctx: &'a Context, f: F) -> BotResult<Message>
    where
        'a: 'b,
        F: Send + FnOnce(CreateMessage<'b>) -> Result<CreateMessage<'b>, CreateMessageError>;

    /// Response with content, embed, attachment, ...
    ///
    /// Includes reaction_delete
    async fn build_response<'a, 'b, F>(&self, ctx: &'a Context, f: F) -> BotResult<()>
    where
        'a: 'b,
        F: Send + FnOnce(CreateMessage<'b>) -> Result<CreateMessage<'b>, CreateMessageError>;

    /// Response with simple content
    async fn respond<C: Into<String> + Send>(
        &self,
        ctx: &Context,
        content: C,
    ) -> BotResult<Message>;

    /// Response with simple content
    ///
    /// Includes reaction_delete
    async fn send_response<C: Into<String> + Send>(
        &self,
        ctx: &Context,
        content: C,
    ) -> BotResult<()>;

    /// Reponse with the given embed
    async fn respond_embed(&self, ctx: &Context, embed: Embed) -> BotResult<Message>;

    /// Response for an error message
    ///
    /// Includes reaction_delete
    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()>;

    /// Response with simple content by tagging the author
    ///
    /// Includes reaction_delete
    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()>;

    /// Give the author 60s to delete the message by reacting with `❌`
    fn reaction_delete(&self, ctx: &Context, owner: UserId);
}

#[async_trait]
impl MessageExt for Message {
    async fn build_response_msg<'a, 'b, F>(&self, ctx: &'a Context, f: F) -> BotResult<Message>
    where
        'a: 'b,
        F: Send + FnOnce(CreateMessage<'b>) -> Result<CreateMessage<'b>, CreateMessageError>,
    {
        f(ctx.http.create_message(self.channel_id))?
            .exec()
            .await?
            .model()
            .await
            .map_err(|e| e.into())
    }

    async fn build_response<'a, 'b, F>(&self, ctx: &'a Context, f: F) -> BotResult<()>
    where
        'a: 'b,
        F: Send + FnOnce(CreateMessage<'b>) -> Result<CreateMessage<'b>, CreateMessageError>,
    {
        f(ctx.http.create_message(self.channel_id))?
            .exec()
            .await?
            .model()
            .await?
            .reaction_delete(ctx, self.author.id);

        Ok(())
    }

    async fn respond<C: Into<String> + Send>(
        &self,
        ctx: &Context,
        content: C,
    ) -> BotResult<Message> {
        let embed = EmbedBuilder::new().description(content).build();

        ctx.http
            .create_message(self.channel_id)
            .embeds(&[embed])?
            .exec()
            .await?
            .model()
            .await
            .map_err(|e| e.into())
    }

    async fn send_response<C: Into<String> + Send>(
        &self,
        ctx: &Context,
        content: C,
    ) -> BotResult<()> {
        self.respond(ctx, content)
            .await?
            .reaction_delete(ctx, self.author.id);

        Ok(())
    }

    async fn respond_embed(&self, ctx: &Context, embed: Embed) -> BotResult<Message> {
        ctx.http
            .create_message(self.channel_id)
            .embeds(&[embed])?
            .exec()
            .await?
            .model()
            .await
            .map_err(|e| e.into())
    }

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let embed = EmbedBuilder::new().color(RED).description(content).build();

        ctx.http
            .create_message(self.channel_id)
            .embeds(&[embed])?
            .exec()
            .await?
            .model()
            .await?
            .reaction_delete(ctx, self.author.id);

        Ok(())
    }

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let embed = EmbedBuilder::new().description(content).build();

        ctx.http
            .create_message(self.channel_id)
            .embeds(&[embed])?
            .reply(self.id)
            .exec()
            .await?
            .model()
            .await?
            .reaction_delete(ctx, self.author.id);

        Ok(())
    }

    fn reaction_delete(&self, ctx: &Context, owner: UserId) {
        let standby = ctx.standby.clone();
        let http = ctx.http.clone();
        let stats = Arc::clone(&ctx.stats);
        let channel_id = self.channel_id;
        let msg_id = self.id;

        let reaction_fut = standby.wait_for_reaction(msg_id, move |event: &ReactionAdd| {
            if event.user_id != owner {
                return false;
            }

            if let ReactionType::Unicode { ref name } = event.0.emoji {
                return name == "❌";
            }

            false
        });

        tokio::spawn(async move {
            let reaction_result = timeout(Duration::from_secs(60), reaction_fut).await;
            stats.message_counts.reaction_deleted_messages.inc();

            if let Ok(Ok(_)) = reaction_result {
                if let Err(why) = http.delete_message(channel_id, msg_id).exec().await {
                    unwind_error!(warn, why, "Error while reaction-deleting msg: {}");
                }
            }
        });
    }
}
