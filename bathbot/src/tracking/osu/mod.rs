use std::collections::HashMap;

use bathbot_psql::Database;
use bathbot_util::IntHasher;
use eyre::{Result, WrapErr};
use papaya::HashMap as PapayaMap;
use require_top::RequireTopScores;
use rosu_v2::{model::GameMode, prelude::Score};
use twilight_model::id::{marker::ChannelMarker, Id};

use self::entry::TrackedUser;
pub use self::{params::TrackEntryParams, stats::OsuTrackingStats};
use crate::core::Context;

mod entry;
mod params;
mod process_score;
mod require_top;
mod stats;

type TrackedUsers = PapayaMap<u32, TrackedUser, IntHasher>;

pub struct OsuTracking {
    users: TrackedUsers,
}

impl OsuTracking {
    // `Context` won't be initialized at this point so we require an explicit
    // `Database` argument.
    pub async fn new(psql: &Database) -> Result<Self> {
        let data = psql
            .select_tracked_osu_users()
            .await
            .wrap_err("Failed to fetch tracked users")?;

        // Populate a regular `HashMap` first and collect it into a `PapayaMap`
        // afterwards so we don't have any initial concurrency overhead.
        let mut users = HashMap::<u32, TrackedUser, IntHasher>::default();

        for user in data {
            users.entry(user.user_id as u32).or_default().insert(user);
        }

        let users = users.into_iter().collect();

        Ok(Self { users })
    }

    pub fn stats() -> OsuTrackingStats {
        OsuTrackingStats::new()
    }

    fn users() -> &'static TrackedUsers {
        &Context::tracking().users
    }

    pub(super) fn process_score(score: Score) {
        let Some(pp) = score.pp else { return };

        let pin = Self::users().pin();

        let Some(user) = pin.get(&score.user_id) else {
            return;
        };

        let Some(entry) = user.get(score.mode) else {
            return;
        };

        if entry.last_pp() > pp {
            return;
        }

        tokio::spawn(process_score::process_score(score, entry));
    }

    pub async fn remove_channel(channel: Id<ChannelMarker>, mode: Option<GameMode>) {
        let channel_id = channel.into_nonzero();

        Self::users().pin().retain(|_, user| {
            user.remove_channel(channel_id, mode);

            !user.is_empty()
        });

        let delete_fut = Context::psql().delete_tracked_osu_channel(channel.get(), mode);

        if let Err(err) = delete_fut.await {
            error!(%channel, ?mode, ?err, "Failed to remove tracked users of channel");
        }
    }

    pub async fn remove_user(user_id: u32, mode: Option<GameMode>, channel: Id<ChannelMarker>) {
        if let Some(user) = Self::users().pin().get(&user_id) {
            user.remove_channel(channel.into_nonzero(), mode)
        }

        let delete_fut = Context::psql().delete_tracked_osu_user(user_id, mode, channel.get());

        if let Err(err) = delete_fut.await {
            error!(user_id, ?mode, %channel, ?err, "Failed to delete tracked user");
        }
    }

    #[must_use = "must call `RequireTopScores::callback`"]
    pub async fn add_user(
        user_id: u32,
        mode: GameMode,
        channel: Id<ChannelMarker>,
        params: TrackEntryParams,
    ) -> Result<Option<RequireTopScores>> {
        let entry = params.into_db_entry(user_id, mode);

        {
            let pin = Self::users().pin();
            let user = pin.get_or_insert_with(user_id, TrackedUser::default);

            let channel_id = channel.into_nonzero();
            user.add(mode, channel_id, params);

            if user.needs_last_pp(mode) {
                return Ok(Some(RequireTopScores::new(entry, channel.get())));
            }
        }

        Context::psql()
            .upsert_tracked_osu_user(&entry, channel.get())
            .await
            .wrap_err("Failed to upsert tracked osu user")?;

        Ok(None)
    }

    pub async fn tracked_users_in_channel(
        channel: Id<ChannelMarker>,
    ) -> Result<Vec<(u32, GameMode, TrackEntryParams)>> {
        let entries = Context::psql()
            .select_tracked_osu_users_channel(channel.get())
            .await
            .wrap_err("Failed to fetch users")?
            .into_iter()
            .map(|entry| {
                let user_id = entry.user_id as u32;
                let mode = GameMode::from(entry.gamemode as u8);
                let params = TrackEntryParams::from(entry);

                (user_id, mode, params)
            })
            .collect();

        Ok(entries)
    }
}
