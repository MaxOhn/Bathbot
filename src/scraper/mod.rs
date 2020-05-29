mod most_played;
mod score;

pub use most_played::MostPlayedMap;
use score::ScraperScores;
pub use score::{ScraperBeatmap, ScraperScore};

use crate::{
    util::{globals::HOMEPAGE, Error, RateLimiter},
    WITH_SCRAPER,
};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Response,
};
use rosu::models::{GameMod, GameMode, GameMods};
use scraper::{Html, Node, Selector};
use std::{collections::HashSet, convert::TryFrom, env, fmt::Write, sync::Mutex};

pub struct Scraper {
    client: Client,
    osu_limiter: Mutex<RateLimiter>,
}

impl Scraper {
    pub async fn new() -> Result<Self, Error> {
        // Initialize client
        let mut builder = Client::builder();
        if WITH_SCRAPER {
            let mut headers = HeaderMap::new();
            let cookie_header = HeaderName::try_from("Cookie").unwrap();
            let cookie_value =
                HeaderValue::from_str(&format!("osu_session={}", env::var("OSU_SESSION")?))?;
            headers.insert(cookie_header, cookie_value);
            builder = builder.default_headers(headers);
            info!("Login Scraper into osu! ...");
        } else {
            debug!("Skipping Scraper login into osu!");
        }
        let client = builder.build()?;
        let osu_limiter = Mutex::new(RateLimiter::new(2, 1));
        Ok(Self {
            client,
            osu_limiter,
        })
    }

    async fn send_request(&self, url: String) -> Result<Response, reqwest::Error> {
        debug!("Scraping url {}", url);
        {
            self.osu_limiter
                .lock()
                .expect("Could not lock osu_limiter")
                .await_access();
        }
        self.client.get(&url).send().await
    }

    // Retrieve the most played maps of a user
    pub async fn get_most_played(
        &self,
        user_id: u32,
        amount: u32,
    ) -> Result<Vec<MostPlayedMap>, Error> {
        let url = format!(
            "{base}users/{id}/beatmapsets/most_played?limit={limit}",
            base = HOMEPAGE,
            id = user_id,
            limit = amount,
        );
        let response = self.send_request(url).await?;
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
    ) -> Result<Vec<ScraperScore>, Error> {
        let mut scores = self._get_leaderboard(map_id, national, mods).await?;
        let mods = mods.and_then(|mods| {
            let dt = GameMod::DoubleTime;
            let nc = GameMod::NightCore;
            if mods.contains(&GameMod::DoubleTime) {
                let mods = mods.as_bits() - dt as u32 + nc as u32;
                Some(GameMods::try_from(mods).unwrap())
            } else if mods.contains(&GameMod::NightCore) {
                let mods = mods.as_bits() - nc as u32 + dt as u32;
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
    ) -> Result<Vec<ScraperScore>, Error> {
        let mut url = format!("{base}beatmaps/{id}/scores?", base = HOMEPAGE, id = map_id);
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
        let response = self.send_request(url).await?;
        let scores: ScraperScores = serde_json::from_slice(&response.bytes().await?)?;
        Ok(scores.get())
    }

    pub async fn get_userid_of_rank(
        &self,
        rank: usize,
        mode: GameMode,
        country_acronym: Option<&str>,
    ) -> Result<u32, Error> {
        if rank < 1 || 10_000 < rank {
            return Err(Error::Custom(format!(
                "Rank must be between 1 and 10_000, got {}",
                rank
            )));
        }
        let mode = get_mode_str(mode);
        let mut url = format!(
            "{base}rankings/{mode}/performance?",
            base = HOMEPAGE,
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
        let response = self.send_request(url).await?;
        let body = match response.error_for_status() {
            Ok(res) => res.text().await?,
            Err(why) => return Err(Error::Custom(format!("Scraper got bad response: {}", why))),
        };
        let html = Html::parse_document(&body);
        let ranking_page_table = Selector::parse(".ranking-page-table").unwrap();
        let ranking_page_table = html.select(&ranking_page_table).next().ok_or_else(|| {
            Error::Custom("No class 'ranking-page-table' found in response".to_string())
        })?;
        let tbody = Selector::parse("tbody").unwrap();
        let tbody = ranking_page_table
            .select(&tbody)
            .next()
            .ok_or_else(|| Error::Custom("No 'tbody' element found in response".to_string()))?;
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
            .ok_or_else(|| Error::Custom("Unwraping 1: Could not find fourth child".to_string()))?
            .children()
            .nth(1)
            .ok_or_else(|| Error::Custom("Unwraping 2: Could not find second child".to_string()))?
            .children()
            .nth(3)
            .ok_or_else(|| Error::Custom("Unwraping 3: Could not find fourth child".to_string()))?;
        match node.value() {
            Node::Element(e) => {
                if let Some(id) = e.attr("data-user-id") {
                    Ok(id.parse::<u32>().unwrap())
                } else {
                    Err(Error::Custom(
                        "Could not find attribute 'data-user-id'".to_string(),
                    ))
                }
            }
            _ => Err(Error::Custom("Did not reach Element node".to_string())),
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
