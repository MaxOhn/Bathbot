use bathbot_psql::Database;
use bathbot_util::CowUtils;
use eyre::{Result, WrapErr};
use rosu_v2::request::UserId;
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::core::Context;

#[derive(Copy, Clone)]
pub struct TwitchManager {
    psql: &'static Database,
}

impl TwitchManager {
    pub fn new() -> Self {
        Self {
            psql: Context::psql(),
        }
    }

    pub async fn id_from_osu(self, user_id: &UserId) -> Result<Option<u64>> {
        match user_id {
            UserId::Id(user_id) => self
                .psql
                .select_twitch_id_by_osu_id(*user_id)
                .await
                .wrap_err("failed to get twitch id by osu id"),
            UserId::Name(username) => {
                let username = username.cow_replace('_', r"\_");

                self.psql
                    .select_twitch_id_by_osu_name(username.as_ref())
                    .await
                    .wrap_err("failed to get twitch id by osu name")
            }
        }
    }

    /// Returns whether a new entry was inserted
    pub async fn track(self, channel: Id<ChannelMarker>, twitch_id: u64) -> Result<bool> {
        self.psql
            .insert_tracked_twitch_stream(channel, twitch_id)
            .await
            .wrap_err("failed to insert twitch stream for tracking")
    }

    /// Returns whether an entry was deleted
    pub async fn untrack(self, channel: Id<ChannelMarker>, twitch_id: u64) -> Result<bool> {
        self.psql
            .delete_tracked_twitch_stream(channel, twitch_id)
            .await
            .wrap_err("failed to remove tracked twitch stream")
    }

    pub async fn untrack_all(self, channel: Id<ChannelMarker>) -> Result<()> {
        self.psql
            .delete_tracked_twitch_streams(channel)
            .await
            .wrap_err("failed to remove tracked twitch streams")
    }
}
