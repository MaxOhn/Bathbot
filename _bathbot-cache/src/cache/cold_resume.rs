use std::{collections::HashMap, hash::BuildHasher};

use bathbot_model::twilight_gateway::SessionsRkyv;
use bb8_redis::redis::{aio::ConnectionLike, AsyncCommands, Cmd};
use eyre::{Result, WrapErr};
use rkyv::with::With;
use tracing::info;
use twilight_gateway::Session;

use crate::{key::RedisKey, model::CachedArchive, Cache};

const STORE_DURATION: u64 = 240;

impl Cache {
    pub async fn freeze<S>(&self, resume_data: &HashMap<u64, Session, S>) -> Result<()> {
        let resume_data = With::<_, SessionsRkyv>::cast(resume_data);
        let bytes =
            rkyv::to_bytes::<_, 128>(resume_data).wrap_err("Failed to serialize resume data")?;

        self.connection()
            .await?
            .set_ex(RedisKey::resume_data(), bytes.as_slice(), STORE_DURATION)
            .await
            .wrap_err("Failed to store resume data bytes")?;

        info!("Successfully froze cache for {STORE_DURATION} seconds");

        Ok(())
    }

    pub async fn defrost<S: BuildHasher + Default>(&self) -> Result<HashMap<u64, Session, S>> {
        let mut conn = self.connection().await?;

        let resume_data_opt: Option<CachedArchive<HashMap<u64, Session, S>>> = conn
            .get(RedisKey::resume_data())
            .await
            .wrap_err("Failed to get stored resume data")?;

        if let Some(resume_data) = resume_data_opt {
            info!("Successfully defrosted cache");

            return Ok(resume_data.deserialize_with::<SessionsRkyv>());
        }

        let mut cmd = Cmd::new();
        cmd.arg("FLUSHDB");

        conn.req_packed_command(&cmd)
            .await
            .wrap_err("Failed to flush redis entries")?;

        info!("Empty resume data, starting with fresh cache");

        Ok(HashMap::with_hasher(S::default()))
    }
}
