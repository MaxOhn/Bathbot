use std::fmt::Write;

use bathbot_model::{
    SnipeCountries, SnipeCountryPlayer, SnipeCountryStatistics, SnipePlayer, SnipeRecent,
    SnipeScore, SnipeScoreParams,
};
use bathbot_util::{
    constants::HUISMETBENEN,
    datetime::{DATE_FORMAT, TIME_FORMAT},
    osu::ModSelection,
};
use eyre::{Result, WrapErr};
use time::{format_description::FormatItem, OffsetDateTime};

use crate::{site::Site, Client};

impl Client {
    pub async fn get_snipe_player(
        &self,
        country: &str,
        user_id: u32,
    ) -> Result<Option<SnipePlayer>> {
        let url = format!(
            "{HUISMETBENEN}player/{country}/{user_id}?type=id",
            country = country.to_lowercase(),
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        SnipePlayer::deserialize(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe player: {body}")
        })
    }

    pub async fn get_snipe_country(&self, country: &str) -> Result<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{HUISMETBENEN}rankings/{country}/pp/weighted",
            country = country.to_lowercase()
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe country: {body}")
        })
    }

    pub async fn get_country_statistics(&self, country: &str) -> Result<SnipeCountryStatistics> {
        let country = country.to_lowercase();
        let url = format!("{HUISMETBENEN}rankings/{country}/statistics");

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize country statistics: {body}")
        })
    }

    pub async fn get_national_snipes(
        &self,
        user_id: u32,
        sniper: bool,
        from: OffsetDateTime,
        until: OffsetDateTime,
    ) -> Result<Vec<SnipeRecent>> {
        pub const DATETIME_FORMAT: &[FormatItem<'_>] = &[
            FormatItem::Compound(DATE_FORMAT),
            FormatItem::Literal(b"T"),
            FormatItem::Compound(TIME_FORMAT),
            FormatItem::Literal(b"Z"),
        ];

        let url = format!(
            "{HUISMETBENEN}changes/{version}/{user_id}?since={since}&until={until}&includeOwnSnipes=false",
            version = if sniper { "new" } else { "old" },
            since = from.format(DATETIME_FORMAT).unwrap(),
            until = until.format(DATETIME_FORMAT).unwrap()
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe recent: {body}")
        })
    }

    pub async fn get_national_firsts(&self, params: &SnipeScoreParams) -> Result<Vec<SnipeScore>> {
        let mut url = format!(
            "{HUISMETBENEN}player/{country}/{user}/topranks?sort={sort}&order={order}&page={page}",
            country = params.country,
            user = params.user_id,
            page = params.page,
            sort = params.order,
            order = if params.descending { "desc" } else { "asc" },
        );

        if let Some(ref mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods.is_empty() {
                    url.push_str("&mods=nomod");
                } else {
                    let _ = write!(url, "&mods={mods}");
                }
            }
        }

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe score: {body}")
        })
    }

    pub async fn get_national_firsts_count(&self, params: &SnipeScoreParams) -> Result<usize> {
        let mut url = format!(
            "{HUISMETBENEN}player/{country}/{user}/topranks/count",
            country = params.country,
            user = params.user_id,
        );

        if let Some(ref mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods.is_empty() {
                    url.push_str("?mods=nomod");
                } else {
                    let _ = write!(url, "?mods={mods}");
                }
            }
        }

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe score count: {body}")
        })
    }

    /// Don't use this; use `RedisManager::snipe_countries` instead.
    pub async fn get_snipe_countries(&self) -> Result<SnipeCountries> {
        let url = "https://api.huismetbenen.nl/country/all?only_with_data=true";
        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize snipe countries: {body}")
        })
    }
}
