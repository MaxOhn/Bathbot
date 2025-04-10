use bathbot_model::{RelaxPlayersDataResponse, RelaxScore};
use bathbot_util::constants::RELAX_API;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::CountryCode;

use crate::{Client, site::Site};

impl Client {
    /// /api/scores
    /// GET relax score leaderboard (a.k.a. highest pp relax scores)
    pub async fn get_relax_score_leaderboard(&self) -> Result<Vec<RelaxScore>> {
        let url = format!("{RELAX_API}/scores");

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax top scores: {body}")
        })
    }

    /// /api/scores/{id}
    /// GET relax score by its ID
    pub async fn get_relax_scores(&self, score_id: u32) -> Result<Vec<RelaxScore>> {
        let url = format!("{RELAX_API}/scores/{score_id}");

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize player's relax scores: {body}")
        })
    }

    /// /api/players
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
    ) -> Result<RelaxPlayersDataResponse> {
        let mut url = format!("{RELAX_API}/players");

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

    /// /api/players/{id}
    /// GET Relax player by osu! ID
    pub async fn get_relax_player(&self, user_id: u32) -> Result<Option<RelaxPlayersDataResponse>> {
        let url = format!("{RELAX_API}/players/{user_id}");

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax player: {body}")
        })
    }

    /// /api/players/{id}/scores
    /// GET all relax scores set by a player
    pub async fn get_relax_player_scores(&self, user_id: u32) -> Result<Vec<RelaxScore>> {
        let url = format!("{RELAX_API}/players/{user_id}/scores");

        let bytes = self.make_get_request(url, Site::Relax).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize relax player scores: {body}")
        })
    }
}
