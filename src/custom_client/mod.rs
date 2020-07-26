mod deserialize;
mod most_played;
mod osu_stats;
mod score;

pub use most_played::MostPlayedMap;
pub use osu_stats::*;
use score::ScraperScores;
pub use score::{ScraperBeatmap, ScraperScore};

use crate::{
    util::{constants::OSU_BASE, error::CustomClientError, osu::ModSelection},
    BotResult,
};

use governor::{
    clock::DefaultClock,
    state::{keyed::DashMapStateStore, StateStore},
    Quota, RateLimiter,
};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    multipart::Form,
    Client, Response,
};
use rosu::models::{GameMode, GameMods};
use scraper::{Html, Node, Selector};
use serde_json::Value;
use std::{collections::HashSet, convert::TryFrom, env, fmt::Write, hash::Hash, num::NonZeroU32};

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
#[allow(clippy::enum_variant_names)]
enum Site {
    OsuWebsite,
    OsuStats,
    OsuHiddenApi,
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

    async fn make_request(&self, url: String, site: Site) -> BotResult<Response> {
        debug!("Scraping url {}", url);
        self.ratelimit(site).await;
        let response = self.client.get(&url).send().await?;
        Ok(response.error_for_status()?)
    }

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
                ModSelection::None => Ok(()),
                ModSelection::Include(mods) => write!(mod_str, "+{}", mods),
                ModSelection::Exclude(mods) => write!(mod_str, "-{}", mods),
                ModSelection::Exact(mods) => write!(mod_str, "!{}", mods),
            };
            form = form.text("mods", mod_str);
        }
        let request = self
            .client
            .post("https://osustats.ppy.sh/api/getScores")
            .multipart(form);
        self.ratelimit(Site::OsuStats).await;
        let response = request.send().await?;
        // let text = response.text().await?;
        // let result: Value = serde_json::from_str(&text)?;
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
        let maps: Vec<MostPlayedMap> = serde_json::from_slice(&response.bytes().await?)?;
        Ok(maps)
    }

    // Retrieve the leaderboard of a map (national / global)
    // If mods contain DT / NC, it will do another request for the opposite
    pub async fn get_leaderboard(
        &self,
        map_id: u32,
        national: bool,
        mods: Option<&GameMods>,
    ) -> BotResult<Vec<ScraperScore>> {
        let mut scores = self._get_leaderboard(map_id, national, mods).await?;
        let mods = mods.and_then(|mods| {
            let dt = GameMods::DoubleTime.bits();
            let nc = GameMods::NightCore.bits();
            if mods.contains(GameMods::DoubleTime) {
                let mods = mods.bits() - dt + nc;
                Some(GameMods::try_from(mods).unwrap())
            } else if mods.contains(GameMods::NightCore) {
                let mods = mods.bits() - nc + dt;
                Some(GameMods::try_from(mods).unwrap())
            } else {
                None
            }
        });
        if mods.is_some() {
            let mut new_scores = self
                ._get_leaderboard(map_id, national, mods.as_ref())
                .await?;
            scores.append(&mut new_scores);
            scores.sort_by(|a, b| b.score.cmp(&a.score));
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
        mods: Option<&GameMods>,
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
        let scores: ScraperScores = serde_json::from_slice(&response.bytes().await?)?;
        Ok(scores.get())
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
        let mode = get_mode_str(mode);
        let mut url = format!(
            "{base}rankings/{mode}/performance?",
            base = OSU_BASE,
            mode = mode,
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
            .ok_or_else(|| CustomClientError::RankingPageTable)?;
        let tbody = Selector::parse("tbody").unwrap();
        let tbody = ranking_page_table
            .select(&tbody)
            .next()
            .ok_or_else(|| CustomClientError::TBody)?;
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
                    Err(CustomClientError::DataUserId.into())
                }
            }
            _ => Err(CustomClientError::DataUserId.into()),
        }
    }
}

fn get_mode_str<'s>(mode: GameMode) -> &'s str {
    match mode {
        GameMode::STD => "osu",
        GameMode::MNA => "mania",
        GameMode::TKO => "taiko",
        GameMode::CTB => "fruits",
    }
}
