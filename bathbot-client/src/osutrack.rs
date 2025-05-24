use std::time::Instant;

use bathbot_model::{OsuTrackHistoryEntry, RankAccPeaks};
use eyre::{Report, Result, WrapErr};
use hyper::{Request, header::USER_AGENT};
use rosu_v2::model::GameMode;

use crate::{Client, ClientError, MY_USER_AGENT, client::Body, metrics::ClientMetrics, site::Site};

impl Client {
    /// Don't use this; use `RedisManager::osutrack_history` instead.
    pub async fn osutrack_user_history(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Result<Vec<OsuTrackHistoryEntry>> {
        // We can also specify `from={from}&to={to}` but while discord
        // doesn't provide datetime options we don't really have a nice
        // interface for users so we don't use it for now
        let url = format!(
            "https://osutrack-api.ameo.dev/stats_history?user={user_id}&mode={mode}",
            mode = mode as u8
        );

        let bytes = self.make_get_request(url, Site::OsuTrack).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize: {body}")
        })
    }

    pub async fn osu_user_rank_acc_peak(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Result<Option<RankAccPeaks>, ClientError> {
        let url = format!(
            "https://osutrack-api.ameo.dev/peak?user={user_id}&mode={mode}",
            mode = mode as u8
        );

        let bytes = self.make_get_request(url, Site::OsuTrack).await?;

        RankAccPeaks::deserialize(&bytes).map_err(|err| {
            let body = String::from_utf8_lossy(&bytes);
            let wrap = format!("Failed to deserialize rank acc peaks: {body}");

            ClientError::Report(Report::new(err).wrap_err(wrap))
        })
    }

    pub async fn notify_osutrack_user_activity(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Result<(), ClientError> {
        let url = format!(
            "https://osutrack-api.ameo.dev/update?user={user_id}&mode={mode}",
            mode = mode as u8
        );

        trace!("POST request to url {url}");

        let req = Request::post(&url)
            .header(USER_AGENT, MY_USER_AGENT)
            .body(Body::default())
            .wrap_err("Failed to build POST request")?;

        let start = Instant::now();

        let response = self.client.request(req).await.map_err(|err| {
            ClientMetrics::internal_error(Site::OsuTrack);

            Report::new(err).wrap_err("Failed to receive POST response")
        })?;

        let status = response.status();

        match status.as_u16() {
            200..=299 => {}
            400 => return Err(ClientError::BadRequest),
            404 => return Err(ClientError::NotFound),
            429 => return Err(ClientError::Ratelimited),
            _ => {
                let err = eyre!("Failed with status code {status} when requesting url {url}");

                return Err(err.into());
            }
        };

        let latency = start.elapsed();
        ClientMetrics::observe(Site::OsuTrack, status, latency);

        Ok(())
    }
}
