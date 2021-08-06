use crate::{
    embeds::EmbedBuilder, util::constants::RED, BotResult, CommandData, CommandDataCompact,
    Context, MessageBuilder,
};

use async_trait::async_trait;
use std::{borrow::Cow, slice};
use twilight_http::Response;
use twilight_model::{
    application::{
        callback::{CallbackData, InteractionResponse},
        interaction::ApplicationCommand,
    },
    channel::Message,
    id::{ChannelId, InteractionId, MessageId},
};

#[async_trait]
pub trait MessageExt {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Option<Response<Message>>>;

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
        response: Option<Response<Message>>,
    ) -> BotResult<Response<Message>>;

    // TODO: add boolean for ephemeral or not
    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()>;

    async fn reply<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()>;
}

#[async_trait]
impl MessageExt for (MessageId, ChannelId) {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Option<Response<Message>>> {
        let mut req = ctx.http.create_message(self.1);

        if let Some(ref content) = builder.content {
            req = req.content(content.as_ref())?;
        }

        if let Some(ref embed) = builder.embed {
            req = req.embeds(slice::from_ref(embed))?;
        }

        Ok(Some(req.exec().await?))
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
        response: Option<Response<Message>>,
    ) -> BotResult<Response<Message>> {
        let msg_id = match response {
            Some(response) => response.model().await?.id,
            None => self.0,
        };

        let mut req = ctx
            .http
            .update_message(self.1, msg_id)
            .content(builder.content.as_ref().map(Cow::as_ref))?;

        if let Some(ref embed) = builder.embed {
            req = req.embeds(slice::from_ref(embed))?;
        }

        Ok(req.exec().await?)
    }

    async fn error<C: Into<String> + Send>(&self, ctx: &Context, content: C) -> BotResult<()> {
        let embed = EmbedBuilder::new().description(content).build();

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
impl<'s> MessageExt for (InteractionId, &'s str) {
    async fn create_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
    ) -> BotResult<Option<Response<Message>>> {
        let response = InteractionResponse::ChannelMessageWithSource(CallbackData {
            allowed_mentions: None,
            content: builder.content.map(Cow::into_owned),
            embeds: builder.embed.map_or_else(Vec::new, |e| vec![e]),
            flags: None,
            tts: None,
        });

        ctx.http
            .interaction_callback(self.0, self.1, &response)
            .exec()
            .await?;

        Ok(None)
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
        _: Option<Response<Message>>,
    ) -> BotResult<Response<Message>> {
        let req = ctx
            .http
            .update_interaction_original(self.1)?
            .content(builder.content.as_ref().map(Cow::as_ref))?
            .embeds(builder.embed.as_ref().map(slice::from_ref))?;

        Ok(req.exec().await?)
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
    ) -> BotResult<Option<Response<Message>>> {
        (self.id, self.channel_id)
            .create_message(ctx, builder)
            .await
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
        response: Option<Response<Message>>,
    ) -> BotResult<Response<Message>> {
        (self.id, self.channel_id)
            .update_message(ctx, builder, response)
            .await
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
    ) -> BotResult<Option<Response<Message>>> {
        (self.id, self.token.as_str())
            .create_message(ctx, builder)
            .await
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
        response: Option<Response<Message>>,
    ) -> BotResult<Response<Message>> {
        (self.id, self.token.as_str())
            .update_message(ctx, builder, response)
            .await
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
    ) -> BotResult<Option<Response<Message>>> {
        match self {
            Self::Message { msg, .. } => msg.create_message(ctx, builder).await,
            Self::Interaction { command } => command.create_message(ctx, builder).await,
        }
    }

    async fn update_message<'c>(
        &self,
        ctx: &Context,
        builder: MessageBuilder<'c>,
        response: Option<Response<Message>>,
    ) -> BotResult<Response<Message>> {
        match self {
            Self::Message { msg, .. } => msg.update_message(ctx, builder, response).await,
            Self::Interaction { command } => command.update_message(ctx, builder, response).await,
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
    ) -> BotResult<Option<Response<Message>>> {
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
        response: Option<Response<Message>>,
    ) -> BotResult<Response<Message>> {
        match self {
            CommandDataCompact::Message { msg_id, channel_id } => {
                (*msg_id, *channel_id)
                    .update_message(ctx, builder, response)
                    .await
            }
            CommandDataCompact::Interaction {
                interaction_id,
                token,
            } => {
                (*interaction_id, token.as_str())
                    .update_message(ctx, builder, response)
                    .await
            }
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
