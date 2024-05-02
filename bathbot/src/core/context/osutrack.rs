use std::time::Duration;

use flurry::HashMap as FlurryMap;
use rosu_v2::model::GameMode;
use time::OffsetDateTime;

use super::Context;

/// Mapping user ids to the last timestamp that osutrack was notified of that
/// user's activity.
pub type OsuTrackUserNotifTimestamps = FlurryMap<(u32, GameMode), OffsetDateTime>;

impl Context {
    pub async fn notify_osutrack_of_user_activity(&self, user_id: u32, mode: GameMode) {
        const DAY: Duration = Duration::from_secs(60 * 60 * 24);

        let key = (user_id, mode);

        let should_notify = match self.data.osutrack_user_notif_timestamps.pin().get(&key) {
            Some(timestamp) => *timestamp < OffsetDateTime::now_utc() - DAY,
            None => true,
        };

        if !should_notify || !cfg!(feature = "notify_osutrack") {
            return;
        }

        let notify_fut = self
            .clients
            .custom
            .notify_osutrack_user_activity(user_id, mode);

        if let Err(err) = notify_fut.await {
            warn!(
                user_id,
                %mode,
                ?err,
                "Failed to notify osutrack of user activity",
            );
        }

        self.data
            .osutrack_user_notif_timestamps
            .pin()
            .insert(key, OffsetDateTime::now_utc());
    }
}
