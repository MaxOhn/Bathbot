use crate::{BotResult, Context};

use std::time::Instant;
use twilight_model::gateway::presence::{ActivityType, Status};

impl Context {
    pub async fn initiate_cold_resume(&self) {
        info!("Preparing for cold resume");
        let activity_result = self
            .set_cluster_activity(
                Status::Idle,
                ActivityType::Watching,
                String::from("an update being deployed, replies might be delayed"),
            )
            .await;
        if let Err(why) = activity_result {
            debug!("Error while updating activity for cold resume: {}", why);
        }

        // Kill the shards and get their resume info
        // DANGER: WE WILL NOT BE GETTING EVENTS FROM THIS POINT ONWARDS, REBOOT REQUIRED
        let resume_data = self.backend.cluster.down_resumable();
        self.cache
            .prepare_cold_resume(
                &self.clients.redis,
                resume_data,
                self.backend.total_shards,
                self.backend.shards_per_cluster,
            )
            .await;
    }

    pub async fn store_configs(&self) -> BotResult<()> {
        let start = Instant::now();
        let guilds = &self.data.guilds;
        let count = self.clients.psql.insert_guilds(guilds).await?;
        let end = Instant::now();
        info!(
            "Stored {} guild configs in {}ms",
            count,
            (end - start).as_millis()
        );
        Ok(())
    }
}
