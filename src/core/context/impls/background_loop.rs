use crate::{Context, CONFIG};

use futures::stream::{FuturesUnordered, StreamExt};
use rosu_v2::prelude::{
    Beatmap,
    RankStatus::{Approved, Loved, Ranked},
};
use std::sync::Arc;
use tokio::{
    fs::remove_file,
    time::{self, Duration},
};

impl Context {
    pub fn map_garbage_collector(&self, map: &Beatmap) -> GarbageCollectMap {
        GarbageCollectMap::new(map)
    }

    pub async fn garbage_collect_all_maps(&self) -> usize {
        let five_seconds = Duration::from_secs(5);

        let mut garbage_collection = self.data.map_garbage_collection.lock();

        if garbage_collection.is_empty() {
            return 0;
        }

        let config = CONFIG.get().unwrap();
        let total = garbage_collection.len();

        let tasks = garbage_collection.drain().map(|map_id| async move {
            let mut map_path = config.map_path.clone();
            map_path.push(format!("{}.osu", map_id));

            match time::timeout(five_seconds, remove_file(map_path)).await {
                Ok(Ok(_)) => None,
                Ok(Err(_)) | Err(_) => Some(map_id),
            }
        });

        let (count, failed) = tasks
            .collect::<FuturesUnordered<_>>()
            .fold((0, Vec::new()), |(count, mut failed), res| async move {
                match res {
                    None => (count + 1, failed),
                    Some(map_id) => {
                        failed.push(map_id);

                        (count, failed)
                    }
                }
            })
            .await;

        if !failed.is_empty() {
            warn!(
                "Failed to garbage collect {}/{} maps: {:?}",
                failed.len(),
                total,
                failed,
            );
        }

        count
    }

    // Current tasks per iteration:
    //   - Deleting .osu files of unranked maps
    #[cold]
    pub async fn background_loop(ctx: Arc<Context>) {
        if cfg!(debug_assertions) {
            info!("Skip background loop on debug");

            return;
        }

        // Once per day
        let mut interval = time::interval(Duration::from_secs(60 * 60 * 24));
        interval.tick().await;

        loop {
            interval.tick().await;
            debug!("[BG] Background iteration...");

            let count = ctx.garbage_collect_all_maps().await;
            debug!("[BG] Garbage collected {} maps", count);
        }
    }
}

pub struct GarbageCollectMap(Option<u32>);

impl GarbageCollectMap {
    pub fn new(map: &Beatmap) -> Self {
        match map.status {
            Ranked | Loved | Approved => Self(None),
            _ => Self(Some(map.map_id)),
        }
    }

    pub async fn execute(self, ctx: &Context) {
        if let Some(map_id) = self.0 {
            let mut lock = ctx.data.map_garbage_collection.lock();

            lock.insert(map_id);
        }
    }
}
