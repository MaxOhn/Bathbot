use super::{PageChange, Pages};
use crate::{
    bail,
    commands::osu::MedalAchieved,
    database::OsuMedal,
    embeds::MedalEmbed,
    util::{send_reaction, Emote},
    BotResult, Context,
};

use hashbrown::HashMap;
use rosu_v2::prelude::{MedalCompact, User};
use std::{borrow::Cow, sync::Arc};
use tokio::time::{sleep, Duration};
use tokio_stream::StreamExt;
use twilight_http::error::ErrorType;
use twilight_model::{
    channel::{Message, Reaction, ReactionType},
    gateway::payload::ReactionAdd,
    id::UserId,
};

pub struct MedalRecentPagination {
    msg: Message,
    pages: Pages,
    ctx: Arc<Context>,
    user: User,
    all_medals: HashMap<u32, OsuMedal>,
    achieved_medals: Vec<MedalCompact>,
    embeds: HashMap<usize, MedalEmbed>,
    maximized: bool,
}

impl MedalRecentPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        all_medals: HashMap<u32, OsuMedal>,
        achieved_medals: Vec<MedalCompact>,
        index: usize,
        embed_data: MedalEmbed,
        maximized: bool,
    ) -> Self {
        let mut embeds = HashMap::new();
        embeds.insert(index, embed_data);
        let mut pages = Pages::new(1, achieved_medals.len());
        pages.index = index.saturating_sub(1);

        Self {
            msg,
            pages,
            ctx,
            user,
            all_medals,
            achieved_medals,
            embeds,
            maximized,
        }
    }

    fn reactions() -> [Emote; 6] {
        [
            Emote::JumpStart,
            Emote::SingleStepBack,
            Emote::Expand,
            Emote::Minimize,
            Emote::SingleStep,
            Emote::JumpEnd,
        ]
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
            if let Err(why) = self.next_page(reaction.0, ctx).await {
                unwind_error!(warn, why, "Error while paginating medalrecent: {}");
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
                    let reaction_reaction = emote.request_reaction();

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

    async fn next_page(&mut self, reaction: Reaction, ctx: &Context) -> BotResult<()> {
        if self.process_reaction(&reaction.emoji) == PageChange::None {
            return Ok(());
        }

        let embed_data = self.build_page().await?;

        let embed = if self.maximized {
            embed_data.maximized()
        } else {
            embed_data.minimized()
        };

        let content = self.content();

        ctx.http
            .update_message(self.msg.channel_id, self.msg.id)
            .content(content.as_deref())?
            .embeds(&[embed.build()])?
            .exec()
            .await?;

        Ok(())
    }

    fn process_reaction(&mut self, reaction: &ReactionType) -> PageChange {
        let change_result = match reaction {
            ReactionType::Custom {
                name: Some(name), ..
            } => match name.as_str() {
                // Move to start
                "jump_start" => (self.index() != 0).then(|| 0),
                // Move one index left
                "single_step_back" => match self.index() {
                    0 => None,
                    idx => Some(idx.saturating_sub(1)),
                },
                // Move one index right
                "single_step" => (self.index() != self.last_index())
                    .then(|| self.last_index().min(self.index() + 1)),
                // Move to end
                "jump_end" => (self.index() != self.last_index()).then(|| self.last_index()),
                // Maximize
                "expand" => {
                    return match self.maximized {
                        false => {
                            self.maximized = true;

                            PageChange::Change
                        }
                        true => PageChange::None,
                    }
                }
                // Minimize
                "minimize" => {
                    return match self.maximized {
                        true => {
                            self.maximized = false;

                            PageChange::Change
                        }
                        false => PageChange::None,
                    }
                }
                _ => return PageChange::None,
            },
            _ => return PageChange::None,
        };

        match change_result {
            Some(index) => {
                self.pages.index = index;

                PageChange::Change
            }
            None => PageChange::None,
        }
    }

    fn content(&self) -> Option<Cow<str>> {
        let idx = self.pages.index + 1;

        let content = match idx % 10 {
            1 if idx == 1 => "Most recent medal:".into(),
            1 if idx != 11 => format!("{}st most recent medal:", idx).into(),
            2 if idx != 12 => format!("{}nd most recent medal:", idx).into(),
            3 if idx != 13 => format!("{}rd most recent medal:", idx).into(),
            _ => format!("{}th most recent medal:", idx).into(),
        };

        Some(content)
    }

    async fn build_page(&mut self) -> BotResult<MedalEmbed> {
        let idx = self.pages.index + 1;

        if !self.embeds.contains_key(&idx) {
            let (medal, achieved_at) = match self.achieved_medals.get(idx - 1) {
                Some(achieved) => {
                    let medal = match self.all_medals.get(&achieved.medal_id) {
                        Some(medal) => medal,
                        None => bail!("Missing medal id {} in DB medals", achieved.medal_id),
                    };

                    match self.ctx.clients.custom.get_osekai_medal(&medal.name).await {
                        Ok(Some(medal)) => (medal, achieved.achieved_at),
                        Ok(None) => bail!("No osekai medal for DB medal `{}`", medal.name),
                        Err(why) => return Err(why.into()),
                    }
                }
                None => bail!(
                    "Medal index out of bounds: {}/{}",
                    idx,
                    self.achieved_medals.len()
                ),
            };

            let achieved = MedalAchieved {
                user: &self.user,
                achieved_at,
                index: idx,
                medal_count: self.achieved_medals.len(),
            };

            let embed_data = MedalEmbed::new(medal, Some(achieved), false);
            self.embeds.insert(idx, embed_data);
        }

        Ok(self.embeds.get(&idx).cloned().unwrap())
    }

    fn index(&self) -> usize {
        self.pages.index
    }

    fn last_index(&self) -> usize {
        self.pages.last_index
    }
}
