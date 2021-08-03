use crate::{embeds::EmbedBuilder, util::constants::RED, BotResult, Context};

use async_trait::async_trait;
use twilight_http::request::channel::message::create_message::{CreateMessage, CreateMessageError};
use twilight_model::channel::{embed::Embed, Message};

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
        f(ctx.http.create_message(self.channel_id))?.exec().await?;

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
        let embed = EmbedBuilder::new().description(content).build();

        ctx.http
            .create_message(self.channel_id)
            .embeds(&[embed])?
            .exec()
            .await?;

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
            .await?;

        Ok(())
    }

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let embed = EmbedBuilder::new().description(content).build();

        ctx.http
            .create_message(self.channel_id)
            .embeds(&[embed])?
            .reply(self.id)
            .exec()
            .await?;

        Ok(())
    }
}
