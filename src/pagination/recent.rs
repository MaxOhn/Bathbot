use super::{Pages, Pagination};

use crate::{
    embeds::{EmbedData, RecentEmbed},
    BotResult, Context,
};

use async_trait::async_trait;
use rosu::models::{
    ApprovalStatus::{Approved, Loved, Qualified, Ranked},
    Beatmap, Score, User,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::channel::Message;

pub struct RecentPagination {
    msg: Message,
    pages: Pages,
    user: User,
    scores: Vec<Score>,
    maps: HashMap<u32, Beatmap>,
    best: Option<Vec<Score>>,
    global: HashMap<u32, Vec<Score>>,
    maps_in_db: HashSet<u32>,
    embed_data: RecentEmbed,
    ctx: Arc<Context>,
}

impl RecentPagination {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        user: User,
        scores: Vec<Score>,
        maps: HashMap<u32, Beatmap>,
        best: Option<Vec<Score>>,
        global: HashMap<u32, Vec<Score>>,
        maps_in_db: HashSet<u32>,
        embed_data: RecentEmbed,
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(5, scores.len()),
            user,
            scores,
            maps,
            best,
            global,
            maps_in_db,
            embed_data,
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
    fn process_data(&mut self, data: &Self::PageData) {
        self.embed_data = data.clone();
    }
    fn content(&self) -> Option<String> {
        Some(format!("Recent score #{}", self.pages.index + 1))
    }
    async fn final_processing(mut self, ctx: &Context) -> BotResult<bool> {
        let msg = self.msg();
        if ctx.remove_msg(msg.id) {
            // Minimize embed
            let embed = self.embed_data.minimize().build()?;
            let _ = ctx
                .http
                .update_message(msg.channel_id, msg.id)
                .embed(embed)?
                .await;

            // Put missing maps into DB
            if self.maps.len() > self.maps_in_db.len() {
                let map_ids = self.maps_in_db.clone();
                let maps: Vec<_> = self
                    .maps
                    .into_iter()
                    .filter(|(id, _)| !map_ids.contains(&id))
                    .map(|(_, map)| map)
                    .collect();
                let psql = &ctx.clients.psql;
                match psql.insert_beatmaps(&maps).await {
                    Ok(n) if n < 2 => {}
                    Ok(n) => info!("Added {} maps to DB", n),
                    Err(why) => warn!("Error while adding maps to DB: {}", why),
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let score = self.scores.get(self.pages.index).unwrap();
        let map_id = score.beatmap_id.unwrap();
        // Make sure map is ready
        #[allow(clippy::clippy::map_entry)]
        if !self.maps.contains_key(&map_id) {
            let osu = &self.ctx.clients.osu;
            let map = score.get_beatmap(osu).await?;
            self.maps.insert(map_id, map);
        }
        let map = self.maps.get(&map_id).unwrap();
        // Make sure map leaderboard is ready
        let valid_global = matches!(map.approval_status, Ranked | Loved | Qualified | Approved);
        #[allow(clippy::clippy::map_entry)]
        if valid_global && !self.global.contains_key(&map.beatmap_id) {
            let global_lb = map.get_global_leaderboard(self.ctx.osu(), 50).await?;
            self.global.insert(map.beatmap_id, global_lb);
        };
        let global_lb = self
            .global
            .get(&map.beatmap_id)
            .map(|global| global.as_slice());
        if self.best.is_none() && map.approval_status == Ranked {
            let user_fut = self.user.get_top_scores(self.ctx.osu(), 100, map.mode);
            match user_fut.await {
                Ok(scores) => self.best = Some(scores),
                Err(why) => warn!(
                    "Error while getting user top scores for recent pagination: {}",
                    why
                ),
            }
        }
        // Create embed data
        RecentEmbed::new(
            &self.ctx,
            &self.user,
            score,
            map,
            self.best.as_deref(),
            global_lb,
        )
        .await
    }
}
