use super::{Pages, Pagination};

use crate::{embeds::RecentData, Error, Osu};

use rosu::models::{Beatmap, Score, User};
use serenity::{
    async_trait,
    cache::Cache,
    model::{channel::Message, id::UserId},
    collector::ReactionCollector,
    prelude::{RwLock, TypeMap, Context},
};
use std::{collections::HashMap, sync::Arc};

pub struct RecentPagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    user: Box<User>,
    scores: Vec<Score>,
    maps: HashMap<u32, Beatmap>,
    best: Vec<Score>,
    global: HashMap<u32, Vec<Score>>,
    cache: Arc<Cache>,
    data: Arc<RwLock<TypeMap>>,
}

impl RecentPagination {
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        user: User,
        scores: Vec<Score>,
        maps: HashMap<u32, Beatmap>,
        best: Vec<Score>,
        global: HashMap<u32, Vec<Score>>,
    ) -> Self {
        let collector = Self::create_collector(ctx, &msg, author, 60).await;
        let cache = Arc::clone(&ctx.cache);
        let data = Arc::clone(&ctx.data);
        Self {
            msg,
            collector,
            pages: Pages::new(5, scores.len()),
            user: Box::new(user),
            scores,
            maps,
            best,
            global,
            cache,
            data,
        }
    }

    pub fn maps(self) -> HashMap<u32, Beatmap> {
        self.maps
    }
}

#[async_trait]
impl Pagination for RecentPagination {
    type PageData = RecentData;
    fn msg(&mut self) -> &mut Message {
        &mut self.msg
    }
    fn collector(&mut self) -> &mut ReactionCollector {
        &mut self.collector
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    fn reactions() -> &'static [&'static str] {
        &["⏮️", "⏪", "◀️", "▶️", "⏩", "⏭️"]
    }
    async fn final_processing(mut self) -> Result<(), Error> {
        // // Minimize embed
        // resp.edit((&cache, &*http), |m| m.embed(|e| embed_data.minimize(e)))
        //     .await?;

        // // Put missing maps into DB
        // let maps = pagination.maps();
        // if maps.len() > map_ids.len() {
        //     let maps: Vec<Beatmap> = maps
        //         .into_iter()
        //         .filter(|(id, _)| !map_ids.contains(&id))
        //         .map(|(_, map)| map)
        //         .collect();
        //     let data = data.read().await;
        //     let mysql = data.get::<MySQL>().unwrap();
        //     if let Err(why) = mysql.insert_beatmaps(maps) {
        //         warn!("Error while adding maps to DB: {}", why);
        //     }
        // }
        todo!()
    }
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        let score = self.scores.get(self.index()).unwrap();
        let map_id = score.beatmap_id.unwrap();
        // Make sure map is ready
        #[allow(clippy::clippy::map_entry)]
        if !self.maps.contains_key(&map_id) {
            let data = self.data.read().await;
            let osu = data.get::<Osu>().unwrap();
            let map = score.get_beatmap(osu).await?;
            self.maps.insert(map_id, map);
        }
        let map = self.maps.get(&map_id).unwrap();
        // Make sure map leaderboard is ready
        #[allow(clippy::clippy::map_entry)]
        if !self.global.contains_key(&map.beatmap_id) {
            let data = self.data.read().await;
            let osu = data.get::<Osu>().unwrap();
            let global_lb = map.get_global_leaderboard(&osu, 50).await?;
            self.global.insert(map.beatmap_id, global_lb);
        };
        let global_lb = self.global.get(&map.beatmap_id).unwrap();
        // Create embed data
        RecentData::new(
            &*self.user,
            score,
            map,
            &self.best,
            &global_lb,
            (&self.cache, &self.data),
        )
        .await
    }
}
