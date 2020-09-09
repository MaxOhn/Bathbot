mod deserialize;
mod most_played;
mod osu_profile;
mod osu_stats;
mod score;
mod snipe;

pub use most_played::MostPlayedMap;
pub use osu_profile::*;
pub use osu_stats::*;
use score::ScraperScores;
pub use score::{ScraperBeatmap, ScraperScore};
pub use snipe::*;

use crate::{
    util::{
        constants::{AVATAR_URL, HUISMETBENEN, OSU_BASE},
        error::CustomClientError,
        osu::ModSelection,
    },
    BotResult,
};

use chrono::{DateTime, Utc};
use futures::future::FutureExt;
use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    multipart::Form,
    Client, Response,
};
use rosu::models::User;
use rosu::models::{GameMode, GameMods};
use scraper::{Html, Node, Selector};
use serde_json::Value;
use std::{collections::HashSet, convert::TryFrom, fmt::Write, hash::Hash, num::NonZeroU32};

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
#[allow(clippy::enum_variant_names)]
enum Site {
    OsuWebsite,
    OsuStats,
    OsuHiddenApi,
    OsuAvatar,
    OsuSnipe,
}

pub struct CustomClient {
    client: Client,
    ratelimiter: RateLimiter<Site, DashMapStateStore<Site>, DefaultClock>,
}

impl CustomClient {
    pub async fn new(osu_session: &str) -> BotResult<Self> {
        let mut builder = Client::builder();
        let mut headers = HeaderMap::new();
        let cookie_header = HeaderName::try_from("Cookie").unwrap();
        let cookie_value = HeaderValue::from_str(&format!("osu_session={}", osu_session)).unwrap();
        headers.insert(cookie_header, cookie_value);
        builder = builder.default_headers(headers);
        info!("Log into osu! account...");
        let client = builder.build()?;

        let quota = Quota::per_second(NonZeroU32::new(2).unwrap());
        let ratelimiter = RateLimiter::dashmap_with_clock(quota, &DefaultClock::default());
        Ok(Self {
            client,
            ratelimiter,
        })
    }

    async fn ratelimit(&self, site: Site) {
        self.ratelimiter.until_key_ready(&site).await
    }

    async fn make_request(&self, url: impl AsRef<str>, site: Site) -> BotResult<Response> {
        let url = url.as_ref();
        debug!("Requesting url {}", url);
        self.ratelimit(site).await;
        let response = self.client.get(url).send().await?;
        Ok(response.error_for_status()?)
    }

    pub async fn get_snipe_player(&self, country: &str, user_id: u32) -> BotResult<SnipePlayer> {
        let url = format!("{}player/{}/{}?type=id", HUISMETBENEN, country, user_id);
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let player: SnipePlayer = serde_json::from_slice(&bytes).map_err(|e| {
            let content = String::from_utf8_lossy(&bytes).into_owned();
            CustomClientError::snipe_player(e, content)
        })?;
        Ok(player)
    }

    pub async fn get_snipe_country(&self, country: &str) -> BotResult<Vec<SnipeCountryPlayer>> {
        let url = format!("{}rankings/{}/pp/weighted", HUISMETBENEN, country);
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let country_players: Vec<SnipeCountryPlayer> =
            serde_json::from_slice(&bytes).map_err(|e| {
                let content = String::from_utf8_lossy(&bytes).into_owned();
                CustomClientError::snipe_country(e, content)
            })?;
        Ok(country_players)
    }

    pub async fn get_country_unplayed_amount(&self, country: &str) -> BotResult<u32> {
        let url = format!(
            "{}beatmaps/unplayed/{}",
            HUISMETBENEN,
            country.to_lowercase()
        );
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let amount = serde_json::from_slice(&bytes)?;
        Ok(amount)
    }

    pub async fn get_country_biggest_difference(
        &self,
        country: &str,
    ) -> BotResult<(SnipeTopDifference, SnipeTopDifference)> {
        let country = country.to_lowercase();
        let url_gain = format!("{}rankings/{}/topgain", HUISMETBENEN, country);
        let url_loss = format!("{}rankings/{}/toploss", HUISMETBENEN, country);
        let gain = self
            .make_request(url_gain, Site::OsuSnipe)
            .then(|res| async {
                match res {
                    Ok(response) => response.bytes().await.map_err(|e| e.into()),
                    Err(why) => Err(why),
                }
            });
        let loss = self
            .make_request(url_loss, Site::OsuSnipe)
            .then(|res| async {
                match res {
                    Ok(response) => response.bytes().await.map_err(|e| e.into()),
                    Err(why) => Err(why),
                }
            });
        let (gain, loss) = tokio::try_join!(gain, loss)?;
        let gain: SnipeTopDifference = serde_json::from_slice(&gain).map_err(|e| {
            let content = String::from_utf8_lossy(&gain).into_owned();
            CustomClientError::snipe_difference(e, content)
        })?;
        let loss: SnipeTopDifference = serde_json::from_slice(&loss).map_err(|e| {
            let content = String::from_utf8_lossy(&loss).into_owned();
            CustomClientError::snipe_difference(e, content)
        })?;
        Ok((gain, loss))
    }

    pub async fn _get_national_snipes(
        &self,
        user: &User,
        sniper: bool,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> BotResult<Vec<SnipeRecent>> {
        let date_format = "%FT%TZ";
        let url = format!(
            "{}snipes/{}/{}?since={}&until={}",
            HUISMETBENEN,
            user.user_id,
            if sniper { "new" } else { "old" },
            from.format(date_format).to_string(),
            until.format(date_format).to_string()
        );
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let snipes: Vec<SnipeRecent> = serde_json::from_slice(&bytes).map_err(|e| {
            let content = String::from_utf8_lossy(&bytes).into_owned();
            CustomClientError::snipe_recent(e, content)
        })?;
        Ok(snipes)
    }

    /// BAD! DO NOT USE YET!
    pub async fn _get_national_firsts(&self, user: &User) -> BotResult<Vec<SnipeScore>> {
        let url = format!(
            "{}player/{}/{}/all",
            HUISMETBENEN,
            user.country.to_lowercase(),
            user.user_id
        );
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let scores: Vec<SnipeScore> = serde_json::from_slice(&bytes).map_err(|e| {
            let content = String::from_utf8_lossy(&bytes).into_owned();
            CustomClientError::snipe_score(e, content)
        })?;
        Ok(scores)
    }

    pub async fn get_country_globals(
        &self,
        params: &OsuStatsListParams,
    ) -> BotResult<Vec<OsuStatsPlayer>> {
        let mut form = Form::new()
            .text("rankMin", params.rank_min.to_string())
            .text("rankMax", params.rank_max.to_string())
            .text("gamemode", (params.mode as u8).to_string())
            .text("page", params.page.to_string());
        if let Some(ref country) = params.country {
            form = form.text("country", country.to_owned());
        }
        let url = "https://osustats.ppy.sh/api/getScoreRanking";
        debug!("Requesting POST from url {} [page {}]", url, params.page);
        let request = self.client.post(url).multipart(form);
        self.ratelimit(Site::OsuStats).await;
        let response = request.send().await?;
        let bytes = response.bytes().await?;
        let players: Vec<OsuStatsPlayer> = serde_json::from_slice(&bytes).map_err(|e| {
            let content = String::from_utf8_lossy(&bytes).into_owned();
            CustomClientError::globals_list(e, content)
        })?;
        Ok(players)
    }

    /// Be sure whitespaces in the username are **not** replaced
    pub async fn get_global_scores(
        &self,
        params: &OsuStatsParams,
    ) -> BotResult<(Vec<OsuStatsScore>, usize)> {
        let mut form = Form::new()
            .text("accMin", params.acc_min.to_string())
            .text("accMax", params.acc_max.to_string())
            .text("rankMin", params.rank_min.to_string())
            .text("rankMax", params.rank_max.to_string())
            .text("gamemode", (params.mode as u8).to_string())
            .text("sortBy", (params.order as u8).to_string())
            .text("sortOrder", (!params.descending as u8).to_string())
            .text("page", params.page.to_string())
            .text("u1", params.username.clone());
        if let Some(selection) = params.mods {
            let mut mod_str = String::with_capacity(3);
            let _ = match selection {
                ModSelection::Include(mods) => write!(mod_str, "+{}", mods),
                ModSelection::Exclude(mods) => write!(mod_str, "-{}", mods),
                ModSelection::Exact(mods) => write!(mod_str, "!{}", mods),
            };
            form = form.text("mods", mod_str);
        }
        let url = "https://osustats.ppy.sh/api/getScores";
        debug!("Requesting POST from url {}", url);
        let request = self.client.post(url).multipart(form);
        self.ratelimit(Site::OsuStats).await;
        let response = request.send().await?;
        let bytes = response.bytes().await?;
        let result: Value = serde_json::from_slice(&bytes)?;
        let (scores, amount) = if let Value::Array(mut array) = result {
            let mut values = array.drain(..2);
            let scores = serde_json::from_value(values.next().unwrap())?;
            let amount = serde_json::from_value(values.next().unwrap())?;
            (scores, amount)
        } else {
            (Vec::new(), 0)
        };
        Ok((scores, amount))
    }

    // Retrieve the most played maps of a user
    pub async fn get_most_played(
        &self,
        user_id: u32,
        amount: u32,
    ) -> BotResult<Vec<MostPlayedMap>> {
        let url = format!(
            "{base}users/{id}/beatmapsets/most_played?limit={limit}",
            base = OSU_BASE,
            id = user_id,
            limit = amount,
        );
        let response = self.make_request(url, Site::OsuWebsite).await?;
        let bytes = response.bytes().await?;
        let maps: Vec<MostPlayedMap> = serde_json::from_slice(&bytes).map_err(|e| {
            let content = String::from_utf8_lossy(&bytes).into_owned();
            CustomClientError::most_played(e, content)
        })?;
        Ok(maps)
    }

    // Retrieve the leaderboard of a map (national / global)
    // If mods contain DT / NC, it will do another request for the opposite
    // If mods dont contains Mirror and its a mania map, it will perform the
    // same requests again but with Mirror enabled
    pub async fn get_leaderboard(
        &self,
        map_id: u32,
        national: bool,
        mods: Option<GameMods>,
        mode: GameMode,
    ) -> BotResult<Vec<ScraperScore>> {
        let mut scores = self._get_leaderboard(map_id, national, mods).await?;
        let non_mirror = mods
            .map(|mods| !mods.contains(GameMods::Mirror))
            .unwrap_or(true);
        if mode == GameMode::MNA && non_mirror {
            let mods = match mods {
                None => Some(GameMods::Mirror),
                Some(mods) => Some(mods | GameMods::Mirror),
            };
            let mut new_scores = self._get_leaderboard(map_id, national, mods).await?;
            scores.append(&mut new_scores);
            scores.sort_unstable_by(|a, b| b.score.cmp(&a.score));
            let mut uniques = HashSet::with_capacity(50);
            scores.retain(|s| uniques.insert(s.user_id));
        }
        let mods = match mods {
            Some(mods) if mods.contains(GameMods::DoubleTime) => Some(mods | GameMods::NightCore),
            Some(mods) if mods.contains(GameMods::NightCore) => {
                Some((mods - GameMods::NightCore) | GameMods::DoubleTime)
            }
            Some(_) | None => None,
        };
        if mods.is_some() {
            if mode == GameMode::MNA && non_mirror {
                let mods = mods.map(|mods| mods | GameMods::Mirror);
                let mut new_scores = self._get_leaderboard(map_id, national, mods).await?;
                scores.append(&mut new_scores);
            }
            let mut new_scores = self._get_leaderboard(map_id, national, mods).await?;
            scores.append(&mut new_scores);
            scores.sort_unstable_by(|a, b| b.score.cmp(&a.score));
            let mut uniques = HashSet::with_capacity(50);
            scores.retain(|s| uniques.insert(s.user_id));
        }
        Ok(scores)
    }

    // Retrieve the leaderboard of a map (national / global)
    async fn _get_leaderboard(
        &self,
        map_id: u32,
        national: bool,
        mods: Option<GameMods>,
    ) -> BotResult<Vec<ScraperScore>> {
        let mut url = format!("{base}beatmaps/{id}/scores?", base = OSU_BASE, id = map_id);
        if national {
            url.push_str("type=country");
        }
        if let Some(mods) = mods {
            if mods.is_empty() {
                url.push_str("&mods[]=NM");
            } else {
                for m in mods.iter() {
                    let _ = write!(url, "&mods[]={}", m);
                }
            }
        }
        let response = self.make_request(url, Site::OsuHiddenApi).await?;
        let bytes = response.bytes().await?;
        let scores: ScraperScores = serde_json::from_slice(&bytes).map_err(|e| {
            let content = String::from_utf8_lossy(&bytes).into_owned();
            CustomClientError::leaderboard(e, content)
        })?;
        Ok(scores.get())
    }

    pub async fn get_avatar(&self, user_id: u32) -> BotResult<Vec<u8>> {
        let url = format!("{}{}", AVATAR_URL, user_id);
        let response = self.make_request(url, Site::OsuAvatar).await?;
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn get_osu_profile(
        &self,
        user_id: u32,
        mode: GameMode,
        with_all_achievements: bool,
    ) -> BotResult<(OsuProfile, OsuAchievements)> {
        let url = format!(
            "{base}users/{user_id}/{mode}",
            base = OSU_BASE,
            user_id = user_id,
            mode = get_mode_str(mode)
        );
        let body = self
            .make_request(url, Site::OsuWebsite)
            .await?
            .text()
            .await?;
        let html = Html::parse_document(&body);
        let user_element = Selector::parse("#json-user").unwrap();
        let json = match html.select(&user_element).next() {
            Some(element) => element.first_child().unwrap().value().as_text().unwrap(),
            None => return Err(CustomClientError::MissingElement("#json-user").into()),
        };
        let user: OsuProfile = serde_json::from_str(json.trim())?;
        let achievements = if with_all_achievements {
            let achievement_element = Selector::parse("#json-achievements").unwrap();
            let json = match html.select(&achievement_element).next() {
                Some(element) => element.first_child().unwrap().value().as_text().unwrap(),
                None => return Err(CustomClientError::MissingElement("#json-achievements").into()),
            };
            serde_json::from_str::<Vec<OsuAchievement>>(json.trim())?.into()
        } else {
            OsuAchievements::default()
        };
        Ok((user, achievements))
    }

    pub async fn get_userid_of_rank(
        &self,
        rank: usize,
        mode: GameMode,
        country_acronym: Option<&str>,
    ) -> BotResult<u32> {
        if rank < 1 || 10_000 < rank {
            return Err(CustomClientError::RankIndex(rank).into());
        }
        let mut url = format!(
            "{base}rankings/{mode}/performance?",
            base = OSU_BASE,
            mode = get_mode_str(mode),
        );
        if let Some(country) = country_acronym {
            let _ = write!(url, "country={}&", country);
        }
        let mut page_idx = rank / 50;
        if rank % 50 != 0 {
            page_idx += 1;
        }
        let _ = write!(url, "page={}", page_idx);
        let body = self
            .make_request(url, Site::OsuWebsite)
            .await?
            .text()
            .await?;
        let html = Html::parse_document(&body);
        let ranking_page_table = Selector::parse(".ranking-page-table").unwrap();
        let ranking_page_table = html
            .select(&ranking_page_table)
            .next()
            .ok_or_else(|| CustomClientError::MissingElement(".ranking-page-table"))?;
        let tbody = Selector::parse("tbody").unwrap();
        let tbody = ranking_page_table
            .select(&tbody)
            .next()
            .ok_or_else(|| CustomClientError::MissingElement("tbody"))?;
        let child = tbody
            .children()
            .enumerate()
            .filter(|(i, _)| i % 2 == 1) // Filter the empty lines
            .map(|(_, child)| child)
            .nth(if rank % 50 == 0 { 49 } else { (rank % 50) - 1 })
            .unwrap();
        let node = child
            .children()
            .nth(3)
            .ok_or_else(|| CustomClientError::RankNode(1))?
            .children()
            .nth(1)
            .ok_or_else(|| CustomClientError::RankNode(2))?
            .children()
            .nth(3)
            .ok_or_else(|| CustomClientError::RankNode(3))?;
        match node.value() {
            Node::Element(e) => {
                if let Some(id) = e.attr("data-user-id") {
                    Ok(id.parse::<u32>().unwrap())
                } else {
                    Err(CustomClientError::MissingElement("attribute data-user-id").into())
                }
            }
            _ => Err(CustomClientError::MissingElement("attribute data-user-id").into()),
        }
    }
}

fn get_mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "osu",
        GameMode::MNA => "mania",
        GameMode::TKO => "taiko",
        GameMode::CTB => "fruits",
    }
}
