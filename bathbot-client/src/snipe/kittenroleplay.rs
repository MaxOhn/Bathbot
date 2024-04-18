use std::fmt::Write;

use bathbot_model::{
    KittenRoleplayCountries, KittenRoleplayCountryRankingPlayer, KittenRoleplayCountryStatistics,
    KittenRoleplayModsCount, KittenRoleplayPlayerHistoryEntry, KittenRoleplayPlayerStatistics,
    KittenRoleplayScore, KittenRoleplaySnipe, KittenRoleplayStarsCount, SnipeCountryListOrder,
    SnipeScoreParams, SnipedWeek,
};
use bathbot_util::osu::ModSelection;
use eyre::{Report, Result, WrapErr};

use crate::{site::Site, Client, ClientError};

pub async fn get_snipe_player(
    client: &Client,
    user_id: u32,
) -> Result<Option<KittenRoleplayPlayerStatistics>> {
    let url = format!(
        "https://snipes.kittenroleplay.com/api/player/statistics?mode=3&user_id={user_id}",
    );

    let bytes = match client.make_get_request(url, Site::KittenRoleplay).await {
        Ok(bytes) => bytes,
        Err(ClientError::NotFound) => return Ok(None),
        Err(err) => return Err(Report::new(err)),
    };

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay player statistics: {body}")
    })
}

pub async fn get_snipe_country(
    client: &Client,
    country_code: &str,
    sort: SnipeCountryListOrder,
) -> Result<Vec<KittenRoleplayCountryRankingPlayer>> {
    let url = format!(
        "https://snipes.kittenroleplay.com/api/country/rankings?mode=3&country={country_code}&sort={sort}",
        sort = sort.as_kittenroleplay_str(),
    );

    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay country ranking: {body}")
    })
}

pub async fn get_country_statistics(
    client: &Client,
    country_code: &str,
) -> Result<KittenRoleplayCountryStatistics> {
    let url = format!(
        "https://snipes.kittenroleplay.com/api/country/statistics?mode=3&country={country_code}"
    );

    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay country statistics: {body}")
    })
}

pub async fn get_sniped_players(
    client: &Client,
    user_id: u32,
    sniper: bool,
) -> Result<Vec<SnipedWeek>> {
    let url = format!(
        "https://snipes.kittenroleplay.com/api/player/{version}/counter?mode=3&user_id={user_id}",
        version = if sniper { "gains" } else { "losses" },
    );

    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay sniped players: {body}")
    })
}

pub async fn get_national_snipes(
    client: &Client,
    user_id: u32,
    sniper: bool,
    offset: u32,
    days_since: u32,
) -> Result<Vec<KittenRoleplaySnipe>> {
    let url = format!(
        "https://snipes.kittenroleplay.com/api/player/{version}?mode=3&\
        user_id={user_id}&since={days_since}&self_snipes=0&offset={offset}&limit=50",
        version = if sniper { "gains" } else { "losses" },
    );

    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay snipes: {body}")
    })
}

pub async fn get_national_firsts(
    client: &Client,
    params: &SnipeScoreParams,
) -> Result<Vec<KittenRoleplayScore>> {
    let mut url = format!(
        "https://snipes.kittenroleplay.com/api/player/scores?mode=3&user_id={user}&sort={sort}\
        &order={order}&offset={offset}",
        user = params.user_id,
        sort = params.order.as_kittenroleplay_str(),
        order = if params.descending { "DESC" } else { "ASC" },
        offset = (params.page as u32 - 1) * 50,
    );

    if let Some(limit) = params.limit {
        let _ = write!(url, "&limit={limit}");
    }

    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay national firsts: {body}")
    })
}

pub async fn get_national_firsts_count(
    client: &Client,
    params: &SnipeScoreParams,
) -> Result<usize> {
    let counts = get_mod_counts(client, params.user_id).await?;

    let count = match params.mods {
        Some(ModSelection::Include(ref mods) | ModSelection::Exact(ref mods)) => {
            let bits = mods.bits();

            counts
                .iter()
                .find_map(|count| (count.mods == bits).then_some(count.count))
                .unwrap_or(0)
        }
        None | Some(ModSelection::Exclude(_)) => {
            counts.iter().fold(0, |sum, count| sum + count.count)
        }
    };

    Ok(count as usize)
}

pub async fn get_countries(client: &Client) -> Result<KittenRoleplayCountries> {
    let url = "https://snipes.kittenroleplay.com/api/country/list?mode=3";
    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay countries: {body}")
    })
}

pub async fn get_mod_counts(client: &Client, user_id: u32) -> Result<Vec<KittenRoleplayModsCount>> {
    let url = format!(
        "https://snipes.kittenroleplay.com/api/player/mods/combos?mode=3&user_id={user_id}",
    );

    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay score count: {body}")
    })
}

pub async fn get_player_history(
    client: &Client,
    user_id: u32,
) -> Result<Vec<KittenRoleplayPlayerHistoryEntry>> {
    let url = format!(
        "https://snipes.kittenroleplay.com/api/player/historical?mode=3&user_id={user_id}",
    );

    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay player history: {body}")
    })
}

pub async fn get_player_stars(
    client: &Client,
    user_id: u32,
) -> Result<Vec<KittenRoleplayStarsCount>> {
    let url =
        format!("https://snipes.kittenroleplay.com/api/player/stars?mode=3&user_id={user_id}",);

    let bytes = client.make_get_request(url, Site::KittenRoleplay).await?;

    serde_json::from_slice(&bytes).wrap_err_with(|| {
        let body = String::from_utf8_lossy(&bytes);

        format!("Failed to deserialize kittenroleplay player stars: {body}")
    })
}

/*
class Allowed:
    Countries = [country.alpha_2 for country in pycountry.countries] + ['XX']
    RankingSorts = ['count', 'average_accuracy', 'average_stars', 'average_pp', 'weighted_pp', 'average_score', 'total_score']
    ScoreSorts = ['created_at', 'score', 'accuracy', 'max_combo', 'pp', 'stars', 'count_miss']
    Orders = ['DESC', 'ASC']

@bp.route("/country/scores", methods=["GET"])
def country_scores(country, mode, sort, order, limit, offset):

@bp.route("/player/gains", methods=["GET"])
def player_gains(user_id, mode, limit, offset):

@bp.route("/player/losses", methods=["GET"])
def player_losses(user_id, mode, limit, offset):

*/
