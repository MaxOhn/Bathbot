use super::{Pages, Pagination};

use crate::{
    embeds::{EmbedData, RecentEmbed},
    BotResult, Context,
};

use async_trait::async_trait;
use hashbrown::HashMap;
use rosu_v2::prelude::{
    BeatmapUserScore, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score, User,
};
use std::sync::Arc;
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::channel::Message;

pub struct RecentPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<Score>,
    best: Option<Vec<Score>>,
    embed_data: Option<RecentEmbed>,
    map_scores: HashMap<u32, BeatmapUserScore>,
    ctx: Arc<Context>,
}

impl RecentPagination {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        scores: Vec<Score>,
        idx: usize,
        best: Option<Vec<Score>>,
        map_scores: HashMap<u32, BeatmapUserScore>,
        embed_data: RecentEmbed,
    ) -> Self {
        let mut pages = Pages::new(1, scores.len());
        pages.index = idx;

        Self {
            msg,
            pages,
            user,
            scores,
            best,
            map_scores,
            embed_data: Some(embed_data),
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for RecentPagination {
    type PageData = RecentEmbed;

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn reactions() -> Vec<RequestReactionType> {
        Self::arrow_reactions_full()
    }

    fn multi_step(&self) -> usize {
        5
    }

    fn process_data(&mut self, data: &Self::PageData) {
        self.embed_data.replace(data.clone());
    }

    fn content(&self) -> Option<String> {
        Some(format!("Recent score #{}", self.pages.index + 1))
    }

    async fn final_processing(mut self, ctx: &Context) -> BotResult<()> {
        // Minimize embed
        let embed = self.embed_data.take().unwrap().minimize().build()?;
        let msg = self.msg();

        let _ = ctx
            .http
            .update_message(msg.channel_id, msg.id)
            .embed(embed)?
            .await;

        // Set maps on garbage collection list if unranked
        for map in self.scores.iter().filter_map(|s| s.map.as_ref()) {
            ctx.map_garbage_collector(map).execute(ctx).await;
        }

        // Store maps in DB
        if let Err(why) = ctx.psql().store_scores_maps(self.scores.iter()).await {
            unwind_error!(warn, why, "Error while storing recent maps in DB: {}");
        }

        Ok(())
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let score = self.scores.get(self.pages.index).unwrap();
        let map = score.map.as_ref().unwrap();
        let map_id = map.map_id;

        if self.best.is_none() && map.status == Ranked {
            let user_fut = self
                .ctx
                .osu()
                .user_scores(self.user.user_id)
                .best()
                .limit(50)
                .mode(score.mode);

            match user_fut.await {
                Ok(scores) => self.best = Some(scores),
                Err(why) => unwind_error!(
                    warn,
                    why,
                    "Error while getting user top scores for recent pagination: {}"
                ),
            }
        }

        // Make sure map leaderboard is ready
        let has_leaderboard = matches!(map.status, Ranked | Loved | Qualified | Approved);

        #[allow(clippy::clippy::map_entry)]
        if !self.map_scores.contains_key(&map_id) && has_leaderboard {
            let score_fut = self
                .ctx
                .osu()
                .beatmap_user_score(map_id, self.user.user_id)
                .mode(map.mode);

            match score_fut.await {
                Ok(score) => {
                    self.map_scores.insert(map_id, score);
                }
                Err(OsuError::NotFound) => {}
                Err(why) => unwind_error!(warn, why, "Error while requesting map score: {}"),
            }
        }

        let map_score = self.map_scores.get(&map_id);

        RecentEmbed::new(&self.user, score, self.best.as_deref(), map_score, true).await
    }
}
