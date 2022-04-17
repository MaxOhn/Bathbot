use crate::{core::AssignRoles, BotResult, Database};

use flurry::HashMap as FlurryMap;
use futures::stream::StreamExt;

type AssignRolesMap = FlurryMap<(u64, u64), AssignRoles>;

impl Database {
    #[cold]
    pub async fn get_role_assigns(&self) -> BotResult<AssignRolesMap> {
        let mut stream = sqlx::query!("SELECT * FROM role_assigns").fetch(&self.pool);
        let assigns = AssignRolesMap::with_capacity(200);

        {
            let aref = assigns.pin();

            while let Some(entry) = stream.next().await.transpose()? {
                let key = (entry.channel_id as u64, entry.message_id as u64);

                let missing = aref
                    .compute_if_present(&key, |_, roles| {
                        let mut roles = roles.to_owned();
                        roles.push(entry.role_id as u64);

                        Some(roles)
                    })
                    .is_none();

                if missing {
                    aref.insert(key, smallvec::smallvec![entry.role_id as u64]);
                }
            }
        }

        Ok(assigns)
    }

    pub async fn add_role_assign(&self, channel: u64, message: u64, role: u64) -> BotResult<()> {
        sqlx::query!(
            "INSERT INTO role_assigns \
            VALUES ($1,$2,$3)\
            ON CONFLICT (channel_id,message_id,role_id) DO NOTHING",
            channel as i64,
            message as i64,
            role as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_role_assign(
        &self,
        channel: u64,
        message: u64,
        role: u64,
    ) -> BotResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM role_assigns WHERE channel_id=$1 AND message_id=$2 AND role_id=$3",
            channel as i64,
            message as i64,
            role as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
