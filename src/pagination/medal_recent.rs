use super::{Pages, Pagination, ReactionVec};
use crate::{
    bail, commands::osu::MedalAchieved, database::OsuMedal, embeds::MedalEmbed, BotResult, Context,
};

use async_trait::async_trait;
use hashbrown::HashMap;
use rosu_v2::prelude::{MedalCompact, User};
use std::{borrow::Cow, sync::Arc};
use twilight_model::channel::Message;

pub struct MedalRecentPagination {
    msg: Message,
    pages: Pages,
    ctx: Arc<Context>,
    user: User,
    all_medals: HashMap<u32, OsuMedal>,
    achieved_medals: Vec<MedalCompact>,
    embeds: HashMap<usize, MedalEmbed>,
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
        }
    }
}

#[async_trait]
impl Pagination for MedalRecentPagination {
    type PageData = MedalEmbed;

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn reactions() -> ReactionVec {
        Self::arrow_reactions_full()
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    fn multi_step(&self) -> usize {
        5 * self.pages.per_page
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

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
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
}
