/// Logs an event and sets its target to `"tracking"`.
macro_rules! log {
    ( $level:ident: $( $arg:tt )* ) => {
        tracing::$level!(target: "tracking", $( $arg )*);
    };
}

use std::{collections::HashMap, sync::RwLock};

use bathbot_psql::Database;
use bathbot_util::{IntHasher, datetime::NAIVE_DATETIME_FORMAT};
use eyre::{Result, WrapErr};
use rosu_v2::{model::GameMode, prelude::Score};
use twilight_model::id::{Id, marker::ChannelMarker};

use self::{entry::TrackedUser, require_top::RequireTopScores};
pub use self::{params::TrackEntryParams, stats::OsuTrackingStats};
use crate::core::Context;

mod entry;
mod params;
mod process_score;
mod require_top;
mod stats;

type TrackedUsers = RwLock<HashMap<u32, TrackedUser, IntHasher>>;

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

        let mut users = HashMap::<u32, TrackedUser, IntHasher>::default();

        for user in data {
            users.entry(user.user_id as u32).or_default().insert(user);
        }

        Ok(Self {
            users: RwLock::new(users),
        })
    }

    pub fn stats() -> OsuTrackingStats {
        OsuTrackingStats::new()
    }

    fn users() -> &'static TrackedUsers {
        &Context::tracking().users
    }

    pub(super) fn process_score(score: Score) {
        let Some(pp) = score.pp else { return };

        let entry_opt = Self::users()
            .read()
            .unwrap()
            .get(&score.user_id)
            .and_then(|user| user.try_get(score.mode));

        let Some(entry) = entry_opt else {
            return;
        };

        let (last_pp, last_updated) = entry.last_entry();

        log!(info:
            user = score.user_id,
            score_id = score.id,
            pp,
            ended_at = %score.ended_at.format(NAIVE_DATETIME_FORMAT).unwrap(),
            last_pp,
            last_ended_at = %last_updated.format(NAIVE_DATETIME_FORMAT).unwrap(),
        );

        if last_pp > pp || last_updated >= score.ended_at {
            return;
        }

        tokio::spawn(process_score::process_score(score, entry));
    }

    pub async fn remove_channel(channel: Id<ChannelMarker>, mode: Option<GameMode>) {
        let channel_id = channel.into_nonzero();

        for user in Self::users().read().unwrap().values() {
            // If the user is no longer tracked in any channel we still keep
            // the user entry so we don't need to perform a write-op. This
            // should be fine since user entries are small so we won't flood
            // the memory and the user entry won't contain channels so there's
            // no overhead to processing scores either.
            user.remove_channel(channel_id, mode);
        }

        let delete_fut = Context::psql().delete_tracked_osu_channel(channel.get(), mode);

        if let Err(err) = delete_fut.await {
            error!(%channel, ?mode, ?err, "Failed to remove tracked users of channel");
        }
    }

    pub async fn remove_user(user_id: u32, mode: Option<GameMode>, channel: Id<ChannelMarker>) {
        if let Some(user) = Self::users().read().unwrap().get(&user_id) {
            user.remove_channel(channel.into_nonzero(), mode);
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

        let user_entry = Self::users()
            .write()
            .unwrap()
            .entry(user_id)
            .or_default()
            .get(mode);

        let channel_id = channel.into_nonzero();
        user_entry.add(channel_id, params);

        if user_entry.needs_last_pp() {
            return Ok(Some(RequireTopScores::new(entry, channel.get())));
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
