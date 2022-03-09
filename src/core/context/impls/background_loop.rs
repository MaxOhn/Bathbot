use std::{io::ErrorKind, mem, num::NonZeroU32, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{
    Beatmap,
    RankStatus::{Approved, Loved, Ranked},
};
use tokio::{
    fs::remove_file,
    time::{self, Duration},
};

use crate::{BotResult, Context, CONFIG};

impl Context {
    pub fn map_garbage_collector(&self, map: &Beatmap) -> GarbageCollectMap {
        GarbageCollectMap::new(map)
    }

    pub async fn garbage_collect_all_maps(&self) -> (usize, usize) {
        let maps_to_delete = {
            let mut garbage_collection = self.data.map_garbage_collection.lock();

            if garbage_collection.is_empty() {
                return (0, 0);
            }

            mem::take(&mut *garbage_collection)
        };

        let config = CONFIG.get().unwrap();
        let total = maps_to_delete.len();
        let five_seconds = Duration::from_secs(5);
        let mut success = 0;
        let mut file_not_found = 0;

        for map_id in maps_to_delete {
            let mut map_path = config.paths.maps.clone();
            map_path.push(format!("{map_id}.osu"));

            match time::timeout(five_seconds, remove_file(map_path)).await {
                Ok(Ok(_)) => success += 1,
                Ok(Err(why)) => match why.kind() {
                    ErrorKind::NotFound => file_not_found += 1,
                    _ => {
                        let wrap = format!("[BG] Failed to delete map {map_id}");
                        let report = Report::new(why).wrap_err(wrap);
                        warn!("{:?}", report);
                    }
                },
                Err(_) => warn!("[BG] Timed out while deleting map {map_id}"),
            }
        }

        if file_not_found > 0 {
            warn!("[BG] Failed to delete {file_not_found} maps due to missing file");
        }

        (success, total)
    }

    // Current tasks per iteration:
    //   - Deleting .osu files of unranked maps
    //   - Retrieve all medals from osekai and store them in DB
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
            info!("[BG] Background iteration...");

            match update_medals(&ctx).await {
                Ok(count) => info!("[BG] Updated {count} medals"),
                Err(why) => {
                    let report = Report::new(why).wrap_err("[BG] failed to update medals");
                    warn!("{:?}", report);
                }
            }

            let (success, total) = ctx.garbage_collect_all_maps().await;
            info!("[BG] Garbage collected {success}/{total} maps");
        }
    }
}

async fn update_medals(ctx: &Context) -> BotResult<usize> {
    let medals = ctx.clients.custom.get_osekai_medals().await?;
    ctx.psql().store_medals(&medals).await?;

    Ok(medals.len())
}

pub struct GarbageCollectMap(Option<NonZeroU32>);

impl GarbageCollectMap {
    pub fn new(map: &Beatmap) -> Self {
        match map.status {
            Ranked | Loved | Approved => Self(None),
            _ => Self(NonZeroU32::new(map.map_id)),
        }
    }

    pub fn execute(self, ctx: &Context) {
        if let Some(map_id) = self.0 {
            let mut lock = ctx.data.map_garbage_collection.lock();

            lock.insert(map_id);
        }
    }
}
