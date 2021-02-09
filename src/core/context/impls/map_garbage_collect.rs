use crate::{unwind_error, Context, CONFIG};

use rosu::model::{
    ApprovalStatus::{Approved, Loved, Qualified, Ranked},
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

    pub async fn garbage_collect_loop(ctx: Arc<Context>) {
        if cfg!(debug_assertions) {
            info!("Skip garbage collection on debug");

            return;
        }

        let mut interval = interval(Duration::from_secs(60 * 60 * 24));
        interval.tick().await;

        loop {
            interval.tick().await;

            let mut lock = ctx.data.map_garbage_collection.lock().await;

            if lock.is_empty() {
                debug!("Garbage collection list empty, continue...");

                continue;
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
                        "Error while removing file of garbage collected map {}: {}",
                        map_id
                    ),
                }
            }

            debug!("Garbage collected {} maps...", count);
        }
    }
}

pub struct GarbageCollectMap(Option<u32>);

impl GarbageCollectMap {
    #[inline]
    pub fn new(map: &Beatmap) -> Self {
        match map.approval_status {
            Ranked | Loved | Approved | Qualified => Self(None),
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
