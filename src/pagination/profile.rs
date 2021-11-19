use super::{PageChange, PaginationResult, ReactionVec};

use crate::{
    commands::osu::{ProfileData, ProfileSize},
    embeds::{EmbedData, ProfileEmbed},
    pagination::ReactionWrapper,
    util::{send_reaction, Emote},
    BotResult, Context,
};

use eyre::Report;
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::StreamExt;
use twilight_gateway::Event;
use twilight_http::error::ErrorType;
use twilight_model::{
    channel::{Message, Reaction, ReactionType},
    id::UserId,
};

pub struct ProfilePagination {
    msg: Message,
    data: ProfileData,
    current_size: ProfileSize,
}

impl ProfilePagination {
    pub fn new(msg: Message, data: ProfileData, kind: ProfileSize) -> Self {
        Self {
            msg,
            data,
            current_size: kind,
        }
    }

    fn reactions() -> ReactionVec {
        smallvec![Emote::Expand, Emote::Minimize]
    }

    pub async fn start(mut self, ctx: &Context, owner: UserId, duration: u64) -> PaginationResult {
        let msg_id = self.msg.id;
        ctx.store_msg(msg_id);
        let reactions = Self::reactions();

        let reaction_stream = {
            for emote in &reactions {
                send_reaction(ctx, &self.msg, *emote).await?;
            }

            ctx.standby
                .wait_for_event_stream(move |event: &Event| match event {
                    Event::ReactionAdd(event) => {
                        event.message_id == msg_id && event.user_id == owner
                    }
                    Event::ReactionRemove(event) => {
                        event.message_id == msg_id && event.user_id == owner
                    }
                    _ => false,
                })
                .map(|event| match event {
                    Event::ReactionAdd(add) => ReactionWrapper::Add(add.0),
                    Event::ReactionRemove(remove) => ReactionWrapper::Remove(remove.0),
                    _ => unreachable!(),
                })
                .timeout(Duration::from_secs(duration))
        };

        tokio::pin!(reaction_stream);

        while let Some(Ok(reaction)) = reaction_stream.next().await {
            if let Err(why) = self.next_page(reaction.into_inner(), ctx).await {
                warn!("{:?}", Report::new(why).wrap_err("error while paginating"));
            }
        }

        let msg = self.msg;

        if !ctx.remove_msg(msg.id) {
            return Ok(());
        }

        let delete_fut = ctx.http.delete_all_reactions(msg.channel_id, msg.id).exec();

        if let Err(why) = delete_fut.await {
            if matches!(why.kind(), ErrorType::Response { status, ..} if status.raw() == 403) {
                sleep(Duration::from_millis(100)).await;

                for emote in &reactions {
                    let reaction_reaction = emote.request_reaction_type();

                    ctx.http
                        .delete_current_user_reaction(msg.channel_id, msg.id, &reaction_reaction)
                        .exec()
                        .await?;
                }
            } else {
                return Err(why.into());
            }
        }

        Ok(())
    }

    async fn next_page(&mut self, reaction: Reaction, ctx: &Context) -> BotResult<PageChange> {
        if self.process_reaction(&reaction.emoji) == PageChange::None {
            return Ok(PageChange::None);
        }

        let embed = ProfileEmbed::get_or_create(ctx, self.current_size, &mut self.data).await;

        ctx.http
            .update_message(self.msg.channel_id, self.msg.id)
            .embeds(&[embed.as_builder().build()])?
            .exec()
            .await?;

        Ok(PageChange::Change)
    }

    fn process_reaction(&mut self, reaction: &ReactionType) -> PageChange {
        match reaction {
            ReactionType::Custom {
                name: Some(name), ..
            } => match name.as_str() {
                "expand" => match self.current_size.expand() {
                    Some(size) => {
                        self.current_size = size;

                        PageChange::Change
                    }
                    None => PageChange::None,
                },
                "minimize" => match self.current_size.minimize() {
                    Some(size) => {
                        self.current_size = size;

                        PageChange::Change
                    }
                    None => PageChange::None,
                },
                _ => PageChange::None,
            },
            _ => PageChange::None,
        }
    }
}
