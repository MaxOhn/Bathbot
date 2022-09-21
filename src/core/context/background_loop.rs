use std::sync::Arc;

use tokio::time::{self, Duration};

use crate::Context;

impl Context {
    // Current tasks per iteration:
    //   - Deleting .osu files of unranked maps
    #[cold]
    pub async fn background_loop(ctx: Arc<Context>) {
        // Once per day
        let mut interval = time::interval(Duration::from_secs(60 * 60 * 24));
        interval.tick().await;

        loop {
            interval.tick().await;
            info!("[BG] Background iteration...");

            let (success, total) = ctx.garbage_collect_all_maps().await;
            info!("[BG] Garbage collected {success}/{total} maps");
        }
    }
}
