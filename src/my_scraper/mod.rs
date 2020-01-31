#![allow(dead_code)] // remove once its used

mod score;

use crate::util::{Error, RateLimiter};

use reqwest::Client;
use rosu::models::{GameMode, GameMods, Score};
use scraper::{Html, Node, Selector};

const COUNTRY_LEADERBOARD_BASE: &str = "http://osu.ppy.sh/rankings/";
const MAP_LEADERBOARD_BASE: &str = "http://osu.ppy.sh/beatmaps/";

pub struct Scraper {
    client: Client,
    osu_limiter: RateLimiter,
}

impl Scraper {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            osu_limiter: RateLimiter::new(1, 1),
        }
    }

    // Impossible to implement as of now since you cant put a custom cookie into a request
    #[allow(unused)]
    pub async fn get_top50_scores(
        &mut self,
        map_id: u32,
        national: bool,
        mods: Option<&GameMods>,
    ) -> Result<Vec<Score>, Error> {
        let mut url = format!("{}{}/scores?", MAP_LEADERBOARD_BASE, map_id);
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
        self.osu_limiter.await_access();
        unimplemented!()
    }

    pub async fn get_userid_of_rank(
        &mut self,
        rank: usize,
        mode: GameMode,
        country_acronym: Option<&str>,
    ) -> Result<u32, Error> {
        assert!(0 < rank && rank <= 50);
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
        let response = self.client.get(&url).send().await?;
        let body = match response.error_for_status() {
            Ok(res) => res.text().await?,
            Err(why) => return Err(Error::Custom(format!("Scraper got bad response: {}", why))),
        };
        let html = Html::parse_document(&body);
        let ranking_page_table = Selector::parse(".ranking-page-table").unwrap();
        let ranking_page_table = html
            .select(&ranking_page_table)
            .nth(0)
            .unwrap_or_else(|| panic!("No class 'ranking-page-table' found in response"));
        let tbody = Selector::parse("tbody").unwrap();
        let tbody = ranking_page_table
            .select(&tbody)
            .nth(0)
            .unwrap_or_else(|| panic!("No 'tbody' element found in response"));
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
            .unwrap_or_else(|| panic!("Unwraping 1: Could not find fourth child"))
            .children()
            .nth(1)
            .unwrap_or_else(|| panic!("Unwraping 2: Could not find second child"))
            .children()
            .nth(3)
            .unwrap_or_else(|| panic!("Unwraping 3: Could not find fourth child"));
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

    pub async fn get_top50_names(
        &mut self,
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
        let response = self.client.get(&url).send().await?;
        let body = match response.error_for_status() {
            Ok(res) => res.text().await?,
            Err(why) => return Err(Error::Custom(format!("Scraper got bad response: {}", why))),
        };
        let html = Html::parse_document(&body);
        let ranking_page_table = Selector::parse(".ranking-page-table").unwrap();
        let ranking_page_table = html
            .select(&ranking_page_table)
            .nth(0)
            .unwrap_or_else(|| panic!("No class 'ranking-page-table' found in response"));
        let tbody = Selector::parse("tbody").unwrap();
        let tbody = ranking_page_table
            .select(&tbody)
            .nth(0)
            .unwrap_or_else(|| panic!("No 'tbody' element found in response"));
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
                .unwrap_or_else(|| panic!("Unwraping 1: Could not find fourth child"))
                .children()
                .nth(1)
                .unwrap_or_else(|| panic!("Unwraping 2: Could not find second child"))
                .children()
                .nth(3)
                .unwrap_or_else(|| panic!("Unwraping 3: Could not find fourth child"))
                .children()
                .nth(0)
                .unwrap_or_else(|| panic!("Unwraping 4: Could not find second child"));
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
