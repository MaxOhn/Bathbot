use crate::{
    embeds::EmbedBuilder, util::constants::RED, BotResult, CommandData, CommandDataCompact,
    Context, MessageBuilder,
};

use std::{borrow::Cow, slice};
use twilight_http::Response;
use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::Message,
    http::attachment::Attachment,
    id::{
        marker::{ChannelMarker, InteractionMarker, MessageMarker},
        Id,
    },
};

#[async_trait]
pub trait MessageExt {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>>;

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>>;

    async fn delete_message(&self, ctx: &Context) -> BotResult<()>;

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()>;

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()>;
}

#[async_trait]
impl MessageExt for (Id<MessageMarker>, Id<ChannelMarker>) {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        let mut req = ctx.http.create_message(self.1);

        if let Some(ref content) = builder.content {
            req = req.content(content.as_ref())?;
        }

        if let Some(ref embed) = builder.embed {
            req = req.embeds(slice::from_ref(embed))?;
        }

        if let Some(components) = builder.components {
            req = req.components(components)?;
        }

        let attachment = builder
            .file
            .map(|(name, bytes)| Attachment::from_bytes(name, bytes));

        match attachment {
            Some(attachment) => Ok(req.attachments(&[attachment]).unwrap().exec().await?),
            None => Ok(req.exec().await?),
        }
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        let mut req = ctx
            .http
            .update_message(self.1, self.0)
            .content(builder.content.as_deref())?
            .components(builder.components)?;

        if let Some(ref embed) = builder.embed {
            req = req.embeds(Some(slice::from_ref(embed)))?;
        }

        Ok(req.exec().await?)
    }

    async fn delete_message(&self, ctx: &Context) -> BotResult<()> {
        ctx.http.delete_message(self.1, self.0).exec().await?;

        Ok(())
    }

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let embed = EmbedBuilder::new().color(RED).description(content).build();

        ctx.http
            .create_message(self.1)
            .embeds(&[embed])?
            .exec()
            .await?;

        Ok(())
    }

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let embed = EmbedBuilder::new().description(content).build();

        ctx.http
            .create_message(self.1)
            .embeds(&[embed])?
            .reply(self.0)
            .exec()
            .await?;

        Ok(())
    }
}

#[async_trait]
impl<'s> MessageExt for (Id<InteractionMarker>, &'s str) {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        let client = ctx.interaction();

        let req = client
            .update_response(self.1)
            .content(builder.content.as_ref().map(Cow::as_ref))?
            .embeds(builder.embed.as_ref().map(slice::from_ref))?
            .components(builder.components)?;

        let attachment = builder
            .file
            .map(|(name, bytes)| Attachment::from_bytes(name, bytes));

        match attachment {
            Some(attachment) => Ok(req.attachments(&[attachment]).unwrap().exec().await?),
            None => Ok(req.exec().await?),
        }
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        let client = ctx.interaction();

        let req = client
            .update_response(self.1)
            .content(builder.content.as_deref())?
            .embeds(builder.embed.as_ref().map(slice::from_ref))?
            .components(builder.components)?;

        Ok(req.exec().await?)
    }

    async fn delete_message(&self, ctx: &Context) -> BotResult<()> {
        ctx.interaction().delete_response(self.1).exec().await?;

        Ok(())
    }

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let embed = EmbedBuilder::new().color(RED).description(content).build();
        let builder = MessageBuilder::new().embed(embed);

        self.create_message(ctx, builder).await.map(|_| ())
    }

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let embed = EmbedBuilder::new().description(content).build();
        let builder = MessageBuilder::new().embed(embed);

        self.create_message(ctx, builder).await.map(|_| ())
    }
}

#[async_trait]
impl MessageExt for Message {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        (self.id, self.channel_id)
            .create_message(ctx, builder)
            .await
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        (self.id, self.channel_id)
            .update_message(ctx, builder)
            .await
    }

    async fn delete_message(&self, ctx: &Context) -> BotResult<()> {
        (self.id, self.channel_id).delete_message(ctx).await
    }

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        (self.id, self.channel_id).error(ctx, content).await
    }

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        (self.id, self.channel_id).reply(ctx, content).await
    }
}

#[async_trait]
impl MessageExt for ApplicationCommand {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        (self.id, self.token.as_str())
            .create_message(ctx, builder)
            .await
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        (self.id, self.token.as_str())
            .update_message(ctx, builder)
            .await
    }

    async fn delete_message(&self, ctx: &Context) -> BotResult<()> {
        (self.id, self.token.as_str()).delete_message(ctx).await
    }

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        (self.id, self.token.as_str()).error(ctx, content).await
    }

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        (self.id, self.token.as_str()).reply(ctx, content).await
    }
}

#[async_trait]
impl<'m> MessageExt for CommandData<'m> {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        match self {
            Self::Message { msg, .. } => msg.create_message(ctx, builder).await,
            Self::Interaction { command } => command.create_message(ctx, builder).await,
        }
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        match self {
            Self::Message { msg, .. } => msg.update_message(ctx, builder).await,
            Self::Interaction { command } => command.update_message(ctx, builder).await,
        }
    }

    async fn delete_message(&self, ctx: &Context) -> BotResult<()> {
        match self {
            Self::Message { msg, .. } => msg.delete_message(ctx).await,
            Self::Interaction { command } => command.delete_message(ctx).await,
        }
    }

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        match self {
            Self::Message { msg, .. } => msg.error(ctx, content).await,
            Self::Interaction { command } => command.error(ctx, content).await,
        }
    }

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        match self {
            Self::Message { msg, .. } => msg.reply(ctx, content).await,
            Self::Interaction { command } => command.reply(ctx, content).await,
        }
    }
}

#[async_trait]
impl MessageExt for CommandDataCompact {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        match self {
            CommandDataCompact::Message { msg_id, channel_id } => {
                (*msg_id, *channel_id).create_message(ctx, builder).await
            }
            CommandDataCompact::Interaction {
                interaction_id,
                token,
            } => {
                (*interaction_id, token.as_str())
                    .create_message(ctx, builder)
                    .await
            }
        }
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Response<Message>> {
        match self {
            CommandDataCompact::Message { msg_id, channel_id } => {
                (*msg_id, *channel_id).update_message(ctx, builder).await
            }
            CommandDataCompact::Interaction {
                interaction_id,
                token,
            } => {
                (*interaction_id, token.as_str())
                    .update_message(ctx, builder)
                    .await
            }
        }
    }

    async fn delete_message(&self, ctx: &Context) -> BotResult<()> {
        match self {
            CommandDataCompact::Message { msg_id, channel_id } => {
                (*msg_id, *channel_id).delete_message(ctx).await
            }
            CommandDataCompact::Interaction {
                interaction_id,
                token,
            } => (*interaction_id, token.as_str()).delete_message(ctx).await,
        }
    }

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        match self {
            CommandDataCompact::Message { msg_id, channel_id } => {
                (*msg_id, *channel_id).error(ctx, content).await
            }
            CommandDataCompact::Interaction {
                interaction_id,
                token,
            } => (*interaction_id, token.as_str()).error(ctx, content).await,
        }
    }

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        match self {
            CommandDataCompact::Message { msg_id, channel_id } => {
                (*msg_id, *channel_id).reply(ctx, content).await
            }
            CommandDataCompact::Interaction {
                interaction_id,
                token,
            } => (*interaction_id, token.as_str()).reply(ctx, content).await,
        }
    }
}
