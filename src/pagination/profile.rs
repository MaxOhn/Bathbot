use super::{PageChange, ReactionVec};

use crate::{
    core::Emote,
    embeds::{EmbedData, ProfileEmbed},
    util::send_reaction,
    BotResult, Context,
};

use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::StreamExt;
use twilight_http::Error;
use twilight_model::{
    channel::{Message, Reaction, ReactionType},
    gateway::payload::ReactionAdd,
    id::UserId,
};

pub struct ProfilePagination {
    msg: Message,
    embed: ProfileEmbed,
    minimized: bool,
}

impl ProfilePagination {
    pub fn new(msg: Message, embed: ProfileEmbed) -> Self {
        Self {
            msg,
            embed,
            minimized: true,
        }
    }

    fn reactions() -> ReactionVec {
        smallvec![Emote::Expand, Emote::Minimize]
    }

    pub async fn start(mut self, ctx: &Context, owner: UserId, duration: u64) -> BotResult<()> {
        ctx.store_msg(self.msg.id);
        let reactions = Self::reactions();

        let reaction_stream = {
            for emote in &reactions {
                send_reaction(ctx, &self.msg, *emote).await?;
            }

            ctx.standby
                .wait_for_reaction_stream(self.msg.id, move |r: &ReactionAdd| r.user_id == owner)
                .timeout(Duration::from_secs(duration))
        };

        tokio::pin!(reaction_stream);

        while let Some(Ok(reaction)) = reaction_stream.next().await {
            match self.next_page(reaction.0, ctx).await {
                Ok(PageChange::Delete) => return Ok(()),
                Ok(_) => {}
                Err(why) => unwind_error!(warn, why, "Error while paginating profile: {}"),
            }
        }

        let msg = self.msg;

        if !ctx.remove_msg(msg.id) {
            return Ok(());
        }

        match ctx.http.delete_all_reactions(msg.channel_id, msg.id).await {
            Ok(_) => {}
            Err(Error::Response { status, .. }) if status.as_u16() == 403 => {
                sleep(Duration::from_millis(100)).await;

                for emote in &reactions {
                    let reaction_reaction = emote.request_reaction();

                    ctx.http
                        .delete_current_user_reaction(msg.channel_id, msg.id, reaction_reaction)
                        .await?;
                }
            }
            Err(why) => return Err(why.into()),
        }

        if !self.minimized {
            let embed = self.embed.into_builder().build();

            ctx.http
                .update_message(msg.channel_id, msg.id)
                .embed(embed)?
                .await?;
        }

        Ok(())
    }

    async fn next_page(&mut self, reaction: Reaction, ctx: &Context) -> BotResult<PageChange> {
        let change = match self.process_reaction(&reaction.emoji).await {
            PageChange::None => PageChange::None,
            PageChange::Change => {
                let builder = if self.minimized {
                    self.embed.as_builder()
                } else {
                    self.embed.expand()
                };

                ctx.http
                    .update_message(self.msg.channel_id, self.msg.id)
                    .embed(builder.build())?
                    .await?;

                PageChange::Change
            }
            PageChange::Delete => {
                ctx.http
                    .delete_message(self.msg.channel_id, self.msg.id)
                    .await?;

                PageChange::Delete
            }
        };

        Ok(change)
    }

    async fn process_reaction(&mut self, reaction: &ReactionType) -> PageChange {
        let change_result = match reaction {
            ReactionType::Custom {
                name: Some(name), ..
            } => match name.as_str() {
                "expand" => match self.minimized {
                    true => Some(false),
                    false => None,
                },
                "minimize" => match self.minimized {
                    true => None,
                    false => Some(true),
                },
                _ => return PageChange::None,
            },
            ReactionType::Unicode { name } if name == "❌" => return PageChange::Delete,
            _ => return PageChange::None,
        };

        match change_result {
            Some(min) => {
                self.minimized = min;

                PageChange::Change
            }
            None => PageChange::None,
        }
    }
}
