use crate::{Context, CONFIG};

use rosu::model::{
    ApprovalStatus::{Approved, Loved, Ranked},
    Beatmap,
};
use std::sync::Arc;
use tokio::{
    fs::remove_file,
    time::{interval, Duration},
};

impl Context {
    #[inline]
    pub fn map_garbage_collector(&self, map: &Beatmap) -> GarbageCollectMap {
        GarbageCollectMap::new(map)
    }

    pub async fn garbage_collect_all_maps(&self) -> usize {
        let mut garbage_collection = self.data.map_garbage_collection.lock().await;

        if garbage_collection.is_empty() {
            return 0;
        }

        let config = CONFIG.get().unwrap();
        let mut count = 0;
        let mut failed = Vec::new();

        for map_id in garbage_collection.drain() {
            let mut map_path = config.map_path.clone();
            map_path.push(format!("{}.osu", map_id));

            match remove_file(map_path).await {
                Ok(_) => count += 1,
                Err(_) => failed.push(map_id),
            }
        }

        if !failed.is_empty() {
            warn!(
                "Failed to garbage collect {} maps: {:?}",
                failed.len(),
                failed
            );
        }

        count
    }

    // Multiple tasks:
    //   - Deleting .osu files of unranked maps
    //   - Store modified guild configs in DB
    pub async fn background_loop(ctx: Arc<Context>) {
        if cfg!(debug_assertions) {
            info!("Skip background loop on debug");

            return;
        }

        // Once per day
        let mut interval = interval(Duration::from_secs(60 * 60 * 24));
        interval.tick().await;

        loop {
            interval.tick().await;

            debug!("[BG] Background iteration...");

            let count = ctx.garbage_collect_all_maps().await;
            debug!("[BG] Garbage collected {} maps", count);

            match ctx.psql().insert_guilds(&ctx.data.guilds).await {
                Ok(n) if n > 0 => debug!("[BG] Stored {} guilds in DB", n),
                Ok(_) => debug!("[BG] No new or modified guilds to store in DB"),
                Err(why) => warn!("[BG] Error while storing guilds in DB: {}", why),
            }
        }
    }
}

pub struct GarbageCollectMap(Option<u32>);

impl GarbageCollectMap {
    #[inline]
    pub fn new(map: &Beatmap) -> Self {
        match map.approval_status {
            Ranked | Loved | Approved => Self(None),
            _ => Self(Some(map.beatmap_id)),
        }
    }

    pub async fn execute(self, ctx: &Context) {
        if let Some(map_id) = self.0 {
            let mut lock = ctx.data.map_garbage_collection.lock().await;

            lock.insert(map_id);
        }
    }
}
