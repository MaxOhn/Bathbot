use eyre::{Result, WrapErr};
use serde::Serialize;

use crate::{site::Site, Client};

impl Client {
    pub async fn miss_analyzer_score_request(&self, guild_id: u64, score_id: u64) -> Result<bool> {
        let url = "http://104.6.255.43:24342/api/scorerequest";

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            guild_id: u64,
            score_id: u64,
        }

        let body = Body { guild_id, score_id };
        let json = serde_json::to_vec(&body).unwrap();

        let bytes = self
            .make_json_post_request(url, Site::MissAnalyzer, json)
            .await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize miss analyzer request: {body}")
        })
    }

    pub async fn miss_analyzer_score_response(
        &self,
        guild_id: u64,
        channel_id: u64,
        message_id: u64,
        score_id: u64,
    ) -> Result<()> {
        let url = "http://104.6.255.43:24342/api/scoreresponse";

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            guild_id: u64,
            channel_id: u64,
            message_id: u64,
            score_id: u64,
        }

        let body = Body {
            guild_id,
            channel_id,
            message_id,
            score_id,
        };
        let json = serde_json::to_vec(&body).unwrap();

        let bytes = self
            .make_json_post_request(url, Site::MissAnalyzer, json)
            .await?;

        let response = String::from_utf8_lossy(&bytes);

        debug!("Miss analyzer response: {response}");

        Ok(())
    }
}
