use std::{borrow::Cow, cmp::Reverse, sync::Arc};

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{MedalCompact, User};
use tokio::time::{sleep, Duration};
use tokio_stream::StreamExt;
use twilight_gateway::Event;
use twilight_http::error::ErrorType;
use twilight_model::{
    channel::{Message, Reaction, ReactionType},
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::osu::MedalAchieved,
    custom_client::{OsekaiComment, OsekaiMap, OsekaiMedal},
    embeds::MedalEmbed,
    pagination::ReactionWrapper,
    util::{send_reaction, Emote},
    BotResult, Context,
};

use super::{PageChange, Pages, PaginationResult};

struct CachedMedal {
    medal: OsekaiMedal,
    map_comment: Option<(Vec<OsekaiMap>, Option<OsekaiComment>)>,
}

impl CachedMedal {
    fn new(medal: OsekaiMedal) -> Self {
        Self {
            medal,
            map_comment: None,
        }
    }
}

impl From<OsekaiMedal> for CachedMedal {
    fn from(medal: OsekaiMedal) -> Self {
        Self::new(medal)
    }
}

pub struct MedalRecentPagination {
    msg: Message,
    pages: Pages,
    ctx: Arc<Context>,
    user: User,
    cached_medals: HashMap<u32, CachedMedal>,
    achieved_medals: Vec<MedalCompact>,
    embeds: HashMap<(usize, bool), MedalEmbed>,
    maximized: bool,
    medals: Vec<OsekaiMedal>,
}

impl MedalRecentPagination {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        initial_medal: OsekaiMedal,
        achieved_medals: Vec<MedalCompact>,
        index: usize,
        embed_data: MedalEmbed,
        medals: Vec<OsekaiMedal>,
    ) -> Self {
        let maximized = false;
        let mut embeds = HashMap::new();
        embeds.insert((index, maximized), embed_data);
        let mut pages = Pages::new(1, achieved_medals.len());
        pages.index = index.saturating_sub(1);
        let mut cached_medals = HashMap::new();
        cached_medals.insert(initial_medal.medal_id, CachedMedal::new(initial_medal));

        Self {
            msg,
            pages,
            ctx,
            user,
            cached_medals,
            achieved_medals,
            embeds,
            maximized,
            medals,
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

    pub async fn start(
        mut self,
        ctx: &Context,
        owner: Id<UserMarker>,
        duration: u64,
    ) -> PaginationResult {
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
            if let Err(err) = self.next_page(reaction.into_inner(), ctx).await {
                warn!("{:?}", Report::new(err).wrap_err("error while paginating"));
            }
        }

        let msg = self.msg;

        if !ctx.remove_msg(msg.id) {
            return Ok(());
        }

        let delete_fut = ctx.http.delete_all_reactions(msg.channel_id, msg.id).exec();

        if let Err(err) = delete_fut.await {
            if matches!(err.kind(), ErrorType::Response { status, ..} if status.raw() == 403) {
                sleep(Duration::from_millis(100)).await;

                for emote in &reactions {
                    let reaction_reaction = emote.request_reaction_type();

                    ctx.http
                        .delete_current_user_reaction(msg.channel_id, msg.id, &reaction_reaction)
                        .exec()
                        .await?;
                }
            } else {
                return Err(err.into());
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
            .embeds(Some(&[embed.build()]))?
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

    fn content(&self) -> Option<Cow<'_, str>> {
        let idx = self.pages.index + 1;

        let content = match idx % 10 {
            1 if idx == 1 => "Most recent medal:".into(),
            1 if idx != 11 => format!("{idx}st most recent medal:").into(),
            2 if idx != 12 => format!("{idx}nd most recent medal:").into(),
            3 if idx != 13 => format!("{idx}rd most recent medal:").into(),
            _ => format!("{idx}th most recent medal:").into(),
        };

        Some(content)
    }

    async fn build_page(&mut self) -> BotResult<MedalEmbed> {
        let idx = self.pages.index + 1;

        if !self.embeds.contains_key(&(idx, self.maximized)) {
            let (medal, achieved_at) = match self.achieved_medals.get(idx - 1) {
                Some(achieved) => match self.cached_medals.get_mut(&achieved.medal_id) {
                    Some(medal) => (medal, achieved.achieved_at),
                    None => match self
                        .medals
                        .iter()
                        .position(|medal| medal.medal_id == achieved.medal_id)
                    {
                        Some(idx) => {
                            let medal = self.medals.swap_remove(idx);

                            let medal = self
                                .cached_medals
                                .entry(medal.medal_id)
                                .or_insert(medal.into());

                            (medal, achieved.achieved_at)
                        }
                        None => bail!("No medal with id `{}`", achieved.medal_id),
                    },
                },
                None => bail!(
                    "Medal index out of bounds: {idx}/{}",
                    self.achieved_medals.len()
                ),
            };

            let achieved = MedalAchieved {
                user: &self.user,
                achieved_at,
                index: idx,
                medal_count: self.achieved_medals.len(),
            };

            let (maps, top_comment) = if self.maximized {
                match medal.map_comment {
                    Some(ref tuple) => tuple.to_owned(),
                    None => {
                        let name = &medal.medal.name;
                        let map_fut = self.ctx.client().get_osekai_beatmaps(name);
                        let comment_fut = self.ctx.client().get_osekai_comments(name);

                        let (mut maps, comments) = match tokio::try_join!(map_fut, comment_fut) {
                            Ok(tuple) => tuple,
                            Err(err) => {
                                let wrap = format!(
                                    "failed to retrieve osekai maps or comments for medal {name}"
                                );
                                let report = Report::new(err).wrap_err(wrap);
                                warn!("{report:?}");

                                (Vec::new(), Vec::new())
                            }
                        };

                        let top_comment = comments
                            .into_iter()
                            .filter(|comment| comment.parent_id == 0)
                            .max_by_key(|comment| comment.vote_sum)
                            .filter(|comment| comment.vote_sum > 0);

                        maps.sort_unstable_by_key(|map| Reverse(map.vote_sum));

                        medal.map_comment.insert((maps, top_comment)).to_owned()
                    }
                }
            } else {
                (Vec::new(), None)
            };

            let medal = medal.medal.to_owned();

            let embed_data = MedalEmbed::new(medal, Some(achieved), maps, top_comment);
            self.embeds.insert((idx, self.maximized), embed_data);
        }

        Ok(self.embeds.get(&(idx, self.maximized)).cloned().unwrap())
    }

    fn index(&self) -> usize {
        self.pages.index
    }

    fn last_index(&self) -> usize {
        self.pages.last_index
    }
}
