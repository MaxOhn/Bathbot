use crate::Context;

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
        let resume_data = self
            .cluster
            .down_resumable()
            .into_iter()
            .map(|(key, value)| (key, (value.session_id, value.sequence)))
            .collect();

        debug!("Received resume data");

        if let Err(why) = self.cache.prepare_cold_resume(resume_data) {
            unwind_error!(error, why, "Failed to prepare cold resume: {}");
        }
    }

    #[cold]
    pub async fn stop_all_games(&self) -> usize {
        let active_games = self.game_channels();

        if active_games.is_empty() {
            return 0;
        }

        let mut count = 0;

        let content = "I'll abort this game because I'm about to reboot, \
            you can start a new game again in just a moment...";

        for channel in active_games {
            match self.stop_game(channel).await {
                Ok(true) => {
                    let _ = self
                        .http
                        .create_message(channel)
                        .content(content)
                        .unwrap()
                        .exec()
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
