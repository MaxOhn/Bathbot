use crate::{core::ColdRebootData, BotResult, Context};

use std::{collections::HashMap, time::Instant};
use twilight::model::gateway::presence::{ActivityType, Status};

impl Context {
    pub async fn initiate_cold_resume(&self) -> BotResult<()> {
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
        let start = Instant::now();
        let mut connection = self.clients.redis.get().await;

        // Kill the shards and get their resume info
        // DANGER: WE WILL NOT BE GETTING EVENTS FROM THIS POINT ONWARDS, REBOOT REQUIRED
        let resume_data = self.backend.cluster.down_resumable().await;
        let (guild_chunks, user_chunks) = self.cache.prepare_cold_resume(&self.clients.redis).await;

        // Prepare resume data
        let map: HashMap<_, _> = resume_data
            .into_iter()
            .map(|(shard_id, info)| (shard_id, (info.session_id, info.sequence)))
            .collect();
        let data = ColdRebootData {
            resume_data: map,
            total_shards: self.backend.total_shards,
            guild_chunks,
            shard_count: self.backend.shards_per_cluster,
            user_chunks,
        };
        connection
            .set_and_expire_seconds(
                "cb_cluster_data",
                &serde_json::to_value(data).unwrap().to_string().into_bytes(),
                180,
            )
            .await
            .unwrap();
        let end = Instant::now();
        info!(
            "Cold resume preparations completed in {}ms",
            (end - start).as_millis()
        );
        Ok(())
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

    pub async fn store_values(&self) -> BotResult<()> {
        let start = Instant::now();
        let mania_pp = &self.data.stored_values.mania_pp;
        let mania_stars = &self.data.stored_values.mania_stars;
        let ctb_pp = &self.data.stored_values.ctb_pp;
        let ctb_stars = &self.data.stored_values.ctb_stars;
        let psql = &self.clients.psql;
        let (mania_pp, mania_stars, ctb_pp, ctb_stars) = tokio::try_join!(
            psql.insert_mania_pp(mania_pp),
            psql.insert_mania_stars(mania_stars),
            psql.insert_ctb_pp(ctb_pp),
            psql.insert_ctb_stars(ctb_stars),
        )?;
        let end = Instant::now();
        info!(
            "Stored {} pp and {} star values in {}ms",
            mania_pp + ctb_pp,
            mania_stars + ctb_stars,
            (end - start).as_millis()
        );
        Ok(())
    }
}
