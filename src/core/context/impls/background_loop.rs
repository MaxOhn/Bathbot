use crate::{unwind_error, Context, CONFIG};

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

    async fn garbage_collect_all_maps(&self) {
        let mut lock = self.data.map_garbage_collection.lock().await;

        if lock.is_empty() {
            debug!("[BG] Garbage collection list is empty");

            return;
        }

        let config = CONFIG.get().unwrap();
        let mut count = 0;

        for map_id in lock.drain() {
            let mut map_path = config.map_path.clone();
            map_path.push(format!("{}.osu", map_id));

            match remove_file(map_path).await {
                Ok(_) => count += 1,
                Err(why) => unwind_error!(
                    warn,
                    why,
                    "[BG] Error while removing file of garbage collected map {}: {}",
                    map_id
                ),
            }
        }

        debug!("[BG] Garbage collected {} maps", count);
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

            ctx.garbage_collect_all_maps().await;

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
