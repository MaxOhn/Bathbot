use std::{collections::BTreeMap, fmt::Write};

use bathbot_model::{
    SnipeCountries, SnipeCountryListOrder, SnipeCountryPlayer, SnipeCountryStatistics, SnipePlayer,
    SnipePlayerHistory, SnipeRecent, SnipeScore, SnipeScoreParams,
};
use bathbot_util::{
    constants::HUISMETBENEN,
    datetime::{DATE_FORMAT, TIME_FORMAT},
    osu::ModSelection,
};
use eyre::{Result, WrapErr};
use time::{Date, OffsetDateTime, format_description::FormatItem};

use crate::{Client, site::Site};

pub async fn get_snipe_player(
    client: &Client,
    country: &str,
    user_id: u32,
) -> Result<Option<SnipePlayer>> {
    let url = format!(
        "{HUISMETBENEN}player/{country}/{user_id}?type=id",
        country = country.to_lowercase(),
    );

    let bytes = client.make_get_request(url, Site::Huismetbenen).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize huismetbenen player statistics: {body}")
    })
}

pub async fn get_snipe_player_history(
    client: &Client,
    country: &str,
    user_id: u32,
) -> Result<BTreeMap<Date, u32>> {
    let url = format!("https://api.huismetbenen.nl/player/{country}/{user_id}/history");

    let bytes = client.make_get_request(url, Site::Huismetbenen).await?;

    SnipePlayerHistory::deserialize(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize huismetbenen player history: {body}")
    })
}

pub async fn get_snipe_country(
    client: &Client,
    country: &str,
    sort: SnipeCountryListOrder,
) -> Result<Vec<SnipeCountryPlayer>> {
    let url = format!(
        "{HUISMETBENEN}rankings/{country}/{sort}",
        country = country.to_lowercase(),
        sort = sort.as_huismetbenen_str(),
    );

    let bytes = client.make_get_request(url, Site::Huismetbenen).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize huismetbenen country ranking: {body}")
    })
}

pub async fn get_country_statistics(
    client: &Client,
    country: &str,
) -> Result<SnipeCountryStatistics> {
    let country = country.to_lowercase();
    let url = format!("{HUISMETBENEN}rankings/{country}/statistics");

    let bytes = client.make_get_request(url, Site::Huismetbenen).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize huismetbenen country statistics: {body}")
    })
}

pub async fn get_national_snipes(
    client: &Client,
    user_id: u32,
    sniper: bool,
    since: OffsetDateTime,
) -> Result<Vec<SnipeRecent>> {
    pub const DATETIME_FORMAT: &[FormatItem<'_>] = &[
        FormatItem::Compound(DATE_FORMAT),
        FormatItem::Literal(b"T"),
        FormatItem::Compound(TIME_FORMAT),
        FormatItem::Literal(b"Z"),
    ];

    let url = format!(
        "{HUISMETBENEN}changes/{version}/{user_id}?since={since}&until={until}&\
        includeOwnSnipes=false",
        version = if sniper { "new" } else { "old" },
        since = since.format(DATETIME_FORMAT).unwrap(),
        until = OffsetDateTime::now_utc().format(DATETIME_FORMAT).unwrap()
    );

    let bytes = client.make_get_request(url, Site::Huismetbenen).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize huismetbenen snipes: {body}")
    })
}

pub async fn get_national_firsts(
    client: &Client,
    params: &SnipeScoreParams,
) -> Result<Vec<SnipeScore>> {
    let mut url = format!(
        "{HUISMETBENEN}player/{country}/{user}/topranks?sort={sort}&order={order}&page={page}",
        country = params.country,
        user = params.user_id,
        page = params.page,
        sort = params.order.as_huismetbenen_str(),
        order = if params.descending { "desc" } else { "asc" },
    );

    if let Some(ModSelection::Include(ref mods) | ModSelection::Exact(ref mods)) = params.mods {
        if mods.is_empty() {
            url.push_str("&mods=nomod");
        } else {
            let _ = write!(url, "&mods={mods}");
        }
    }

    let bytes = client.make_get_request(url, Site::Huismetbenen).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize huismetbenen national firsts: {body}")
    })
}

pub async fn get_national_firsts_count(
    client: &Client,
    params: &SnipeScoreParams,
) -> Result<usize> {
    let mut url = format!(
        "{HUISMETBENEN}player/{country}/{user}/topranks/count",
        country = params.country,
        user = params.user_id,
    );

    if let Some(ModSelection::Include(ref mods) | ModSelection::Exact(ref mods)) = params.mods {
        if mods.is_empty() {
            url.push_str("?mods=nomod");
        } else {
            let _ = write!(url, "?mods={mods}");
        }
    }

    let bytes = client.make_get_request(url, Site::Huismetbenen).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize huismetbenen score count: {body}")
    })
}

pub async fn get_countries(client: &Client) -> Result<SnipeCountries> {
    let url = "https://api.huismetbenen.nl/country/all?only_with_data=true";
    let bytes = client.make_get_request(url, Site::Huismetbenen).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize huismetbenen countries: {body}")
    })
}
