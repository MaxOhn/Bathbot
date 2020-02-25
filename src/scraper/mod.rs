#![allow(unused_imports)]

mod score;

use score::ScraperScores;
pub use score::{ScraperBeatmap, ScraperScore};

use crate::util::{Error, RateLimiter};
use reqwest::{multipart::Form, Client, Response};
use rosu::models::{GameMode, GameMods};
use scraper::{Html, Node, Selector};
use std::{env, sync::Mutex};

const COUNTRY_LEADERBOARD_BASE: &str = "http://osu.ppy.sh/rankings/";
const MAP_LEADERBOARD_BASE: &str = "http://osu.ppy.sh/beatmaps/";

pub struct Scraper {
    client: Client,
    osu_limiter: Mutex<RateLimiter>,
}

impl Scraper {
    pub async fn new() -> Result<Self, Error> {
        // Initialize client
        let client = Client::builder().cookie_store(true).build()?;
        /*
        // Prepare osu login
        let form = Form::new()
            .text("username", env::var("OSU_LOGIN_USERNAME")?)
            .text("password", env::var("OSU_LOGIN_PASSWORD")?);
        // Retrieve osu_session cookie by logging in
        let response = client
            .post("https://osu.ppy.sh/session")
            .multipart(form)
            .send()
            .await?;
        // Check if the cookie was received
        let success = response
            .cookies()
            .any(|cookie| cookie.name() == "osu_session");
        if !success {
            return Err(Error::Custom(
                "No osu_session cookie in scraper response".to_string(),
            ));
        }
        info!("Scraper successfully logged into osu!");
        */
        info!("Skipping Scraper login into osu!");
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

    pub async fn get_leaderboard(
        &self,
        map_id: u32,
        national: bool,
        mods: Option<&GameMods>,
    ) -> Result<Vec<ScraperScore>, Error> {
        let mut url = format!(
            "{base}{id}/scores?",
            base = MAP_LEADERBOARD_BASE,
            id = map_id
        );
        if national {
            url.push_str("type=country");
        }
        if let Some(mods) = mods {
            if mods.is_empty() {
                url.push_str("&mods[]=NM");
            } else {
                for m in mods.as_ref() {
                    url.push_str("&mods[]=");
                    url.push_str(&m.to_string());
                }
            }
        }
        let response = self.send_request(url).await?;
        let scores: ScraperScores = serde_json::from_slice(&response.bytes().await?)?;
        Ok(scores.get())
    }

    #[allow(dead_code)]
    pub async fn get_userid_of_rank(
        &self,
        rank: usize,
        mode: GameMode,
        country_acronym: Option<&str>,
    ) -> Result<u32, Error> {
        if rank < 1 || 50 < rank {
            return Err(Error::Custom(format!(
                "Rank must be between 1 and 50, got {}",
                rank
            )));
        }
        let mode = get_mode_str(mode);
        let mut url = format!(
            "{base}{mode}/performance?",
            base = COUNTRY_LEADERBOARD_BASE,
            mode = mode,
        );
        if let Some(country) = country_acronym {
            url.push_str("country=");
            url.push_str(country);
            url.push('&');
        }
        let mut page_idx = rank / 50;
        if rank % 50 != 0 {
            page_idx += 1;
        }
        url.push_str("page=");
        url.push_str(&page_idx.to_string());
        let response = self.send_request(url).await?;
        let body = match response.error_for_status() {
            Ok(res) => res.text().await?,
            Err(why) => return Err(Error::Custom(format!("Scraper got bad response: {}", why))),
        };
        let html = Html::parse_document(&body);
        let ranking_page_table = Selector::parse(".ranking-page-table").unwrap();
        let ranking_page_table = html.select(&ranking_page_table).nth(0).ok_or_else(|| {
            Error::Custom("No class 'ranking-page-table' found in response".to_string())
        })?;
        let tbody = Selector::parse("tbody").unwrap();
        let tbody = ranking_page_table
            .select(&tbody)
            .nth(0)
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

    #[allow(dead_code)]
    pub async fn get_top50_names(
        &self,
        country_acrynom: &str,
        mode: GameMode,
    ) -> Result<Vec<String>, Error> {
        let mode = get_mode_str(mode);
        let url = format!(
            "{base}{mode}/performance?country={country}",
            base = COUNTRY_LEADERBOARD_BASE,
            mode = mode,
            country = country_acrynom
        );
        let response = self.send_request(url).await?;
        let body = match response.error_for_status() {
            Ok(res) => res.text().await?,
            Err(why) => return Err(Error::Custom(format!("Scraper got bad response: {}", why))),
        };
        let html = Html::parse_document(&body);
        let ranking_page_table = Selector::parse(".ranking-page-table").unwrap();
        let ranking_page_table = html.select(&ranking_page_table).nth(0).ok_or_else(|| {
            Error::Custom("No class 'ranking-page-table' found in response".to_string())
        })?;
        let tbody = Selector::parse("tbody").unwrap();
        let tbody = ranking_page_table
            .select(&tbody)
            .nth(0)
            .ok_or_else(|| Error::Custom("No 'tbody' element found in response".to_string()))?;
        let children = tbody
            .children()
            .enumerate()
            .filter(|(i, _)| i % 2 == 1) // Filter the empty lines
            .map(|(_, child)| child);
        let mut names = Vec::new();
        for child in children {
            let node = child
                .children()
                .nth(3)
                .ok_or_else(|| {
                    Error::Custom("Unwraping 1: Could not find fourth child".to_string())
                })?
                .children()
                .nth(1)
                .ok_or_else(|| {
                    Error::Custom("Unwraping 2: Could not find second child".to_string())
                })?
                .children()
                .nth(3)
                .ok_or_else(|| {
                    Error::Custom("Unwraping 3: Could not find fourth child".to_string())
                })?
                .children()
                .nth(0)
                .ok_or_else(|| {
                    Error::Custom("Unwraping 4: Could not find first child".to_string())
                })?;
            let name = match node.value() {
                Node::Text(t) => t.text.trim(),
                _ => return Err(Error::Custom("Did not reach Text node".to_string())),
            };
            names.push(name.to_owned());
        }
        Ok(names)
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
