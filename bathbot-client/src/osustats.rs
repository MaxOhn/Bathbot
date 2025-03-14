use std::time::Duration;

use bathbot_model::{
    ModeAsSeed, OsuStatsBestScores, OsuStatsBestTimeframe, OsuStatsParams, OsuStatsPlayer,
    OsuStatsPlayersArgs, OsuStatsScoresRaw,
};
use bathbot_util::osu::ModSelection;
use bytes::Bytes;
use eyre::{Report, Result, WrapErr};
use itoa::Buffer as IntBuffer;
use rosu_v2::model::GameMode;
use ryu::Buffer as FloatBuffer;
use serde::de::DeserializeSeed;

use crate::{Client, ClientError, multipart::Multipart, site::Site};

const TIMEOUT: Duration = Duration::from_secs(15);

impl Client {
    pub async fn get_country_globals(
        &self,
        params: &OsuStatsPlayersArgs,
    ) -> Result<Vec<OsuStatsPlayer>> {
        let mut buf = IntBuffer::new();
        let mut form = Multipart::new();

        form.push_int("rankMin", params.min_rank, &mut buf)
            .push_int("rankMax", params.max_rank, &mut buf)
            .push_int("gamemode", params.mode as u8, &mut buf)
            .push_int("page", params.page, &mut buf);

        if let Some(ref country) = params.country {
            form.push_text("country", country.as_str());
        }

        let url = "https://osustats.ppy.sh/api/getScoreRanking";
        let post_fut = self.make_multipart_post_request(url, Site::OsuStats, form);

        let bytes = match tokio::time::timeout(TIMEOUT, post_fut).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(err)) => return Err(Report::new(err)),
            Err(_) => bail!("Timeout while waiting for osustats players"),
        };

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize osustats players: {body}")
        })
    }

    /// Be sure whitespaces in the username are **not** replaced
    pub async fn get_global_scores(&self, params: &OsuStatsParams) -> Result<OsuStatsScoresRaw> {
        let mut int_buf = IntBuffer::new();
        let mut float_buf = FloatBuffer::new();
        let mut form = Multipart::new();

        form.push_float("accMin", params.min_acc, &mut float_buf)
            .push_float("accMax", params.max_acc, &mut float_buf)
            .push_int("rankMin", params.min_rank, &mut int_buf)
            .push_int("rankMax", params.max_rank, &mut int_buf)
            .push_int("gamemode", params.mode as u8, &mut int_buf)
            .push_int("sortBy", params.order as u8, &mut int_buf)
            .push_int("sortOrder", !params.descending as u8, &mut int_buf)
            .push_int("page", params.page, &mut int_buf)
            .push_text("u1", params.username.as_str());

        if let Some(selection) = params.get_mods() {
            let mod_str = match selection {
                ModSelection::Include(mods) if mods.is_empty() => "!NM".to_owned(),
                ModSelection::Include(mods) => format!("+{mods}"),
                ModSelection::Exclude { mods, .. } => format!("-{mods}"),
                ModSelection::Exact(mods) => format!("!{mods}"),
            };

            form.push_text("mods", mod_str);
        }

        let url = "https://osustats.ppy.sh/api/getScores";
        let post_fut = self.make_multipart_post_request(url, Site::OsuStats, form);

        let bytes = match tokio::time::timeout(TIMEOUT, post_fut).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(ClientError::BadRequest)) => Bytes::from_static(b"[[],0,true,true]"),
            Ok(Err(err)) => return Err(Report::new(err)),
            Err(_) => bail!("Timeout while waiting for osustats scores"),
        };

        Ok(OsuStatsScoresRaw::new(params.mode, bytes.into()))
    }

    /// Don't use this; use `RedisManager::osustats_best` instead.
    pub async fn get_osustats_best(
        &self,
        timeframe: OsuStatsBestTimeframe,
        mode: GameMode,
    ) -> Result<OsuStatsBestScores> {
        let mut buf = IntBuffer::new();
        let mut form = Multipart::new();

        form.push_int("gamemode", mode as u8, &mut buf)
            .push_int("amount", 100, &mut buf)
            .push_int("duration", timeframe as u8, &mut buf);

        let url = "https://osustats.ppy.sh/api/getBestDayScores";
        let post_fut = self.make_multipart_post_request(url, Site::OsuStats, form);

        let bytes = match tokio::time::timeout(TIMEOUT, post_fut).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(err)) => return Err(Report::new(err)),
            Err(_) => bail!("Timeout while waiting for osustats recentbest"),
        };

        let mut deserializer = serde_json::Deserializer::from_slice(&bytes);

        ModeAsSeed::<OsuStatsBestScores>::new(mode)
            .deserialize(&mut deserializer)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize osustats recentbest: {body}")
            })
    }
}
