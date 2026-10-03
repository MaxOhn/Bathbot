use std::{collections::HashMap, convert::Infallible, hash::BuildHasher};

use bathbot_model::twilight::session::{ArchivedSessions, SessionsRkyv};
use bb8_redis::redis::AsyncCommands;
use eyre::{Result, WrapErr};
use rkyv::{
    rancor::{BoxedError, Panic, ResultExt},
    util::AlignedVec,
    with::With,
};
use tracing::info;
use twilight_gateway::Session;

use crate::{Cache, key::RedisKey};

const STORE_DURATION: u64 = 240;

impl Cache {
    pub async fn freeze<S>(&self, sessions: &HashMap<u32, Session, S>) -> Result<()> {
        info!(len = sessions.len(), "Freezing sessions...");

        let sessions = With::<_, SessionsRkyv>::cast(sessions);

        let bytes = rkyv::api::high::to_bytes_in::<_, BoxedError>(sessions, AlignedVec::<8>::new())
            .wrap_err("Failed to serialize sessions")?;

        self.connection()
            .await?
            .set_ex::<_, _, ()>(RedisKey::resume_data(), bytes.as_slice(), STORE_DURATION)
            .await
            .wrap_err("Failed to store resume data bytes")?;

        info!("Successfully froze cache for {STORE_DURATION} seconds");

        Ok(())
    }

    pub async fn defrost<S: BuildHasher + Default>(
        &self,
    ) -> Result<Option<HashMap<u32, Session, S>>> {
        let mut conn = self.connection().await?;

        let bytes: Vec<u8> = conn
            .get(RedisKey::resume_data())
            .await
            .wrap_err("Failed to get stored resume data")?;

        if bytes.is_empty() {
            // Keep existing cache entries; guild creates refresh the data that
            // applies to us, and stale data is better than missing data
            info!("Sessions not found; keeping existing cache entries");

            return Ok(None);
        }

        let archived = rkyv::access::<ArchivedSessions, Panic>(&bytes).always_ok();

        let sessions = rkyv::api::deserialize_using::<_, _, Infallible>(
            With::<_, SessionsRkyv>::cast(archived),
            &mut (),
        );

        Ok(Some(sessions.always_ok()))
    }
}
