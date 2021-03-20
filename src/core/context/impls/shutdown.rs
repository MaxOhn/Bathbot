use crate::{BotResult, Context};

use std::time::Instant;
use twilight_model::gateway::presence::{ActivityType, Status};

impl Context {
    #[cold]
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

    #[cold]
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

    #[cold]
    pub async fn stop_all_games(&self) -> usize {
        let active_games = self.game_channels();

        if active_games.is_empty() {
            return 0;
        }

        let mut count = 0;

        let content = "I'm about to reboot, you can start a \
                            new game again in just a moment...";

        for channel in active_games {
            match self.stop_game(channel).await {
                Ok(true) => {
                    let _ = self
                        .http
                        .create_message(channel)
                        .content(content)
                        .unwrap()
                        .await;

                    count += 1;
                }
                Ok(false) => {}
                Err(why) => unwind_error!(
                    warn,
                    why,
                    "Error while stopping bg game in channel {}: {}",
                    channel
                ),
            }
        }

        count
    }
}
