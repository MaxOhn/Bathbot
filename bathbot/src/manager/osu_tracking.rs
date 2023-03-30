use std::{collections::HashMap, num::NonZeroU64};

use bathbot_psql::{
    model::osu::{TrackedOsuUserKey, TrackedOsuUserValue},
    Database,
};
use bathbot_util::IntHasher;
use eyre::{Result, WrapErr};
use twilight_model::id::{marker::ChannelMarker, Id};

#[derive(Copy, Clone)]
pub struct OsuTrackingManager<'d> {
    psql: &'d Database,
}

impl<'d> OsuTrackingManager<'d> {
    pub fn new(psql: &'d Database) -> Self {
        Self { psql }
    }

    pub async fn get_users(
        &self,
    ) -> Result<Vec<(TrackedOsuUserKey, TrackedOsuUserValue<IntHasher>)>> {
        self.psql
            .select_tracked_osu_users()
            .await
            .wrap_err("failed to get tracked osu users")
    }

    pub async fn update_date(self, key: TrackedOsuUserKey) -> Result<()> {
        let TrackedOsuUserKey { user_id, mode } = key;

        self.psql
            .update_tracked_osu_user_date(user_id, mode)
            .await
            .wrap_err("failed to update date for tracking")
    }

    pub async fn update_channels(
        self,
        key: TrackedOsuUserKey,
        channels: &HashMap<NonZeroU64, u8, IntHasher>,
    ) -> Result<()> {
        let TrackedOsuUserKey { user_id, mode } = key;

        self.psql
            .update_tracked_osu_user_channels(user_id, mode, channels)
            .await
            .wrap_err("failed to update channels for user in osu tracking")
    }

    pub async fn remove_user(self, key: TrackedOsuUserKey) -> Result<()> {
        let TrackedOsuUserKey { user_id, mode } = key;

        self.psql
            .delete_tracked_osu_user_by_mode(user_id, mode)
            .await
            .wrap_err("failed to remove tracked user by mode")
    }

    pub async fn insert_user(
        self,
        key: TrackedOsuUserKey,
        channel: Id<ChannelMarker>,
        limit: u8,
    ) -> Result<()> {
        let TrackedOsuUserKey { user_id, mode } = key;

        self.psql
            .insert_osu_tracking::<IntHasher>(user_id, mode, channel.into_nonzero(), limit)
            .await
            .wrap_err("failed to insert tracked user")
    }
}
