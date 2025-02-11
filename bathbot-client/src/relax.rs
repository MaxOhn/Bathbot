use crate::{site::Site, Client};
use bathbot_model::{RelaxPlayersDataResponse, RelaxScore, RelaxStatsResponse, RelaxUser};
use bathbot_util::constants::RELAX_API;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::CountryCode;

impl Client {
    /// GET relax score leaderboard (a.k.a. highest pp relax scores)
    pub async fn get_relax_score_leaderboard(&self) -> Result<Vec<RelaxScore>> {
        let url = format!("{}/scores", RELAX_API);

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax top scores: {body}")
        })
    }

    /// GET relax scores set within the last 24 hours
    pub async fn get_relax_recent_scores(&self) -> Result<Vec<RelaxScore>> {
        let url = format!("{}/scores/recent", RELAX_API);

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize recent relax scores: {body}")
        })
    }

    /// GET relax score by its ID
    pub async fn get_relax_scores(&self, user_id: u32) -> Result<Vec<RelaxScore>> {
        let url = format!("{}/scores/{}", RELAX_API, user_id);

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize player's relax scores: {body}")
        })
    }
    /// GET Relax player list
    /// Ordered by total pp
    /// page: page index
    /// country_code: country code to get country leaderboards
    /// search: search query
    pub async fn get_relax_players(
        &self,
        page: Option<u32>,
        country_code: Option<CountryCode>,
        search: Option<String>,
    ) -> Result<Vec<RelaxUser>> {
        let mut url = format!("{}/players", RELAX_API);

        if let Some(p) = page {
            url.push_str(&format!("page={p}&"));
        }

        if let Some(cc) = country_code {
            url.push_str(&format!("countryCode={cc}&"));
        }

        if let Some(q) = search {
            url.push_str(&format!("search={q}&"));
        }

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax players: {body}")
        })
    }

    /// GET Relax player by osu! ID
    pub async fn get_relax_player(&self, id: u32) -> Result<RelaxPlayersDataResponse> {
        let url = format!("{}/players/{}", RELAX_API, id);
        debug!(url);

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax player: {body}")
        })
    }

    /// GET all relax scores set by a player
    pub async fn get_relax_player_scores(&self, id: u32) -> Result<Vec<RelaxScore>> {
        let url = format!("{}/players/{}/scores", RELAX_API, id);

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax player scores: {body}")
        })
    }

    /// GET all relax scores set by a player within the past 24 hours
    pub async fn get_relax_recent_player_scores(&self, id: u32) -> Result<Vec<RelaxScore>> {
        let url = format!("{}/players/{}/scores/recent", RELAX_API, id);

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax recent player scores: {body}")
        })
    }

    pub async fn get_relax_statistics(&self) -> Result<RelaxStatsResponse> {
        let url = format!("{}/stats", RELAX_API);

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax statistics: {body}")
        })
    }
}
