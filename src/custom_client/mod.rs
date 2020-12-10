mod deserialize;
mod most_played;
mod osekai;
mod osu_daily;
mod osu_profile;
mod osu_stats;
mod score;
mod snipe;

pub use most_played::MostPlayedMap;
pub use osekai::*;
pub use osu_daily::*;
pub use osu_profile::*;
pub use osu_stats::*;
use score::ScraperScores;
pub use score::{ScraperBeatmap, ScraperScore};
pub use snipe::*;

use crate::{
    util::{
        constants::{AVATAR_URL, HUISMETBENEN, OSEKAI_MEDAL_API, OSU_BASE, OSU_DAILY_API},
        error::CustomClientError,
        numbers::round,
        osu::ModSelection,
    },
    BotResult, CONFIG,
};

use chrono::{DateTime, Utc};
use cow_utils::CowUtils;
use futures::future::FutureExt;
use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    multipart::Form,
    Client, Response, StatusCode,
};
use rosu::model::{GameMode, GameMods, User};
use scraper::{Html, Node, Selector};
use serde_json::Value;
use std::{
    collections::HashSet,
    convert::TryFrom,
    fmt::{self, Write},
    hash::Hash,
    num::NonZeroU32,
    str::FromStr,
};
use tokio::time::{delay_for, timeout, Duration};

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), ", ", env!("CARGO_PKG_VERSION"));

type ClientResult<T> = Result<T, CustomClientError>;

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
#[allow(clippy::enum_variant_names)]
enum Site {
    OsuWebsite,
    OsuStats,
    OsuHiddenApi,
    OsuAvatar,
    OsuSnipe,
    Osekai,
    OsuDaily,
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
        builder = builder.user_agent(USER_AGENT).default_headers(headers);
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

    async fn make_request(&self, url: impl AsRef<str>, site: Site) -> ClientResult<Response> {
        let url = url.as_ref();
        debug!("Requesting url {}", url);
        self.ratelimit(site).await;
        let response = self.client.get(url).send().await?;
        Ok(response.error_for_status()?)
    }

    pub async fn get_osekai_medal(&self, medal_name: &str) -> ClientResult<Option<OsekaiMedal>> {
        let medal = medal_name.cow_replace(' ', "+");
        let url = format!("{}get_medal?medal={}", OSEKAI_MEDAL_API, medal);
        let response = self.make_request(url, Site::Osekai).await?;
        let bytes = response.bytes().await?;
        let medal: Option<OsekaiMedal> = match serde_json::from_slice(&bytes) {
            Ok(medal) => Some(medal),
            Err(source) => match serde_json::from_slice::<Value>(&bytes)
                .map(|mut v| v.get_mut("error").map(Value::take))
            {
                Ok(Some(Value::String(msg))) if msg == "Medal could not be found" => None,
                _ => {
                    return Err(CustomClientError::Parsing {
                        body: String::from_utf8_lossy(&bytes).into_owned(),
                        source,
                        request: "osekai medal",
                    })
                }
            },
        };
        Ok(medal)
    }

    pub async fn get_snipe_player(&self, country: &str, user_id: u32) -> ClientResult<SnipePlayer> {
        let url = format!(
            "{}player/{}/{}?type=id",
            HUISMETBENEN,
            country.to_lowercase(),
            user_id
        );
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let player: SnipePlayer =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe player",
            })?;
        Ok(player)
    }

    pub async fn get_snipe_country(&self, country: &str) -> ClientResult<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{}rankings/{}/pp/weighted",
            HUISMETBENEN,
            country.to_lowercase()
        );
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let country_players: Vec<SnipeCountryPlayer> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe country",
            })?;
        Ok(country_players)
    }

    pub async fn get_country_unplayed_amount(&self, country: &str) -> ClientResult<u32> {
        let url = format!(
            "{}beatmaps/unplayed/{}",
            HUISMETBENEN,
            country.to_lowercase()
        );
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let amount =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "country unplayed amount",
            })?;
        Ok(amount)
    }

    pub async fn get_country_biggest_difference(
        &self,
        country: &str,
    ) -> ClientResult<(SnipeTopDifference, SnipeTopDifference)> {
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
        let gain: SnipeTopDifference =
            serde_json::from_slice(&gain).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&gain).into_owned(),
                source,
                request: "snipe difference",
            })?;
        let loss: SnipeTopDifference =
            serde_json::from_slice(&loss).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&loss).into_owned(),
                source,
                request: "snipe difference",
            })?;
        Ok((gain, loss))
    }

    pub async fn get_national_snipes(
        &self,
        user: &User,
        sniper: bool,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> ClientResult<Vec<SnipeRecent>> {
        let date_format = "%FT%TZ";
        let url = format!(
            "{}snipes/{}/{}?since={}&until={}",
            HUISMETBENEN,
            user.user_id,
            if sniper { "new" } else { "old" },
            from.format(date_format),
            until.format(date_format)
        );
        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let snipes: Vec<SnipeRecent> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe recent",
            })?;
        Ok(snipes)
    }

    pub async fn get_national_firsts(
        &self,
        params: &SnipeScoreParams,
    ) -> ClientResult<Vec<SnipeScore>> {
        let mut url = format!(
            "{base}player/{country}/{user}/topranks?page={page}&mode={mode}&sort={sort}&order={order}",
            base = HUISMETBENEN,
            country = params.country,
            user = params.user_id,
            page = params.page,
            mode = params.mode,
            sort = params.order,
            order = if params.descending { "desc" } else { "asc" },
        );
        if let Some(mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods == GameMods::NoMod {
                    url.push_str("&mods=nomod");
                } else {
                    let _ = write!(url, "&mods={}", mods);
                }
            }
        }

        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let scores: Vec<SnipeScore> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe score",
            })?;
        Ok(scores)
    }

    pub async fn get_national_firsts_count(
        &self,
        params: &SnipeScoreParams,
    ) -> ClientResult<usize> {
        let mut url = format!(
            "{base}player/{country}/{user}/topranks/count?mode={mode}",
            base = HUISMETBENEN,
            country = params.country,
            user = params.user_id,
            mode = params.mode,
        );
        if let Some(mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods == GameMods::NoMod {
                    url.push_str("&mods=nomod");
                } else {
                    let _ = write!(url, "&mods={}", mods);
                }
            }
        }

        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;
        let count: usize =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe score count",
            })?;
        Ok(count)
    }

    pub async fn get_country_globals(
        &self,
        params: &OsuStatsListParams,
    ) -> ClientResult<Vec<OsuStatsPlayer>> {
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
        let response = match timeout(Duration::from_secs(4), request.send()).await {
            Ok(result) => result?,
            Err(_) => return Err(CustomClientError::OsuStatsTimeout),
        };
        let bytes = response.bytes().await?;
        let players: Vec<OsuStatsPlayer> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "globals list",
            })?;
        Ok(players)
    }

    /// Be sure whitespaces in the username are **not** replaced
    pub async fn get_global_scores(
        &self,
        params: &OsuStatsParams,
    ) -> ClientResult<(Vec<OsuStatsScore>, usize)> {
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
        let response = match timeout(Duration::from_secs(4), request.send()).await {
            Ok(result) => result?,
            Err(_) => return Err(CustomClientError::OsuStatsTimeout),
        };
        let bytes = response.bytes().await?;
        let result: Value =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "osu stats global",
            })?;
        let (scores, amount) = if let Value::Array(mut array) = result {
            let mut values = array.drain(..2);
            let scores = serde_json::from_value(values.next().unwrap()).map_err(|source| {
                CustomClientError::Parsing {
                    body: String::from_utf8_lossy(&bytes).into_owned(),
                    source,
                    request: "osu stats global scores",
                }
            })?;
            let amount = serde_json::from_value(values.next().unwrap()).map_err(|source| {
                CustomClientError::Parsing {
                    body: String::from_utf8_lossy(&bytes).into_owned(),
                    source,
                    request: "osu stats global amount",
                }
            })?;
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
    ) -> ClientResult<Vec<MostPlayedMap>> {
        let url = format!(
            "{base}users/{id}/beatmapsets/most_played?limit={limit}",
            base = OSU_BASE,
            id = user_id,
            limit = amount,
        );
        let response = self.make_request(url, Site::OsuWebsite).await?;
        let bytes = response.bytes().await?;
        let maps: Vec<MostPlayedMap> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "most played",
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
    ) -> ClientResult<Vec<ScraperScore>> {
        let mut scores = self._get_leaderboard(map_id, national, mods).await?;
        let non_mirror = mods
            .map(|mods| !mods.contains(GameMods::Mirror))
            .unwrap_or(true);
        // Check another request for mania's MR is needed
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
            scores.truncate(50);
        }
        // Check if DT / NC is included
        let mods = match mods {
            Some(mods) if mods.contains(GameMods::DoubleTime) => Some(mods | GameMods::NightCore),
            Some(mods) if mods.contains(GameMods::NightCore) => {
                Some((mods - GameMods::NightCore) | GameMods::DoubleTime)
            }
            Some(_) | None => None,
        };
        // If DT / NC included, make another request
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
            scores.truncate(50);
        }
        Ok(scores)
    }

    // Retrieve the leaderboard of a map (national / global)
    async fn _get_leaderboard(
        &self,
        map_id: u32,
        national: bool,
        mods: Option<GameMods>,
    ) -> ClientResult<Vec<ScraperScore>> {
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
        let scores: ScraperScores =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "leaderboard",
            })?;
        Ok(scores.get())
    }

    pub async fn get_avatar(&self, user_id: u32) -> ClientResult<Vec<u8>> {
        let url = format!("{}{}", AVATAR_URL, user_id);
        let response = self.make_request(url, Site::OsuAvatar).await?;
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn get_osu_profile(
        &self,
        user_id: u32,
        mode: GameMode,
        with_all_medals: bool,
    ) -> ClientResult<(OsuProfile, OsuMedals)> {
        let url = format!(
            "{base}users/{user_id}/{mode}",
            base = OSU_BASE,
            user_id = user_id,
            mode = mode
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
            None => return Err(CustomClientError::MissingElement("#json-user")),
        };
        let user: OsuProfile =
            serde_json::from_str(json.trim()).map_err(|source| CustomClientError::Parsing {
                body: json.to_string(),
                source,
                request: "osu profile",
            })?;
        let medals = if with_all_medals {
            let medal_element = Selector::parse("#json-achievements").unwrap();
            let json = match html.select(&medal_element).next() {
                Some(element) => element.first_child().unwrap().value().as_text().unwrap(),
                None => return Err(CustomClientError::MissingElement("#json-achievements")),
            };
            serde_json::from_str::<Vec<OsuMedal>>(json.trim())
                .map_err(|source| CustomClientError::Parsing {
                    body: json.to_string(),
                    source,
                    request: "osu medals",
                })?
                .into()
        } else {
            OsuMedals::default()
        };
        Ok((user, medals))
    }

    pub async fn get_rank_data(&self, mode: GameMode, param: RankParam) -> ClientResult<RankPP> {
        let key = &CONFIG.get().unwrap().tokens.osu_daily;
        let mut url = format!("{}pp?k={}&m={}&", OSU_DAILY_API, key, mode as u8);
        let _ = match param {
            RankParam::Rank(rank) => write!(url, "t=rank&v={}", rank),
            RankParam::Pp(pp) => write!(url, "t=pp&v={}", round(pp)),
        };

        const SECOND: Duration = Duration::from_secs(1);

        let response = loop {
            let response = self.make_request(&url, Site::OsuDaily).await?;

            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                break response;
            }

            debug!("Ratelimited by osudaily, wait a second");
            delay_for(SECOND).await;
        };

        let bytes = response.bytes().await?;
        let rank_pp =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "rank data",
            })?;

        Ok(rank_pp)
    }

    pub async fn get_userid_of_rank(
        &self,
        rank: usize,
        ranking: RankLeaderboard<'_>,
    ) -> ClientResult<u32> {
        if rank < 1 || 10_000 < rank {
            return Err(CustomClientError::RankIndex(rank));
        }

        let mut url = format!(
            "{base}rankings/{mode}/",
            base = OSU_BASE,
            mode = ranking.variant
        );

        match ranking.leaderboard {
            LeaderboardType::Score => url += "score?",
            LeaderboardType::Pp { country } => {
                url += "performance?";

                if let Some(country) = country {
                    let _ = write!(url, "country={}&", country);
                }
            }
        }

        if let GameModeVariant::Mania(Some(variant)) = ranking.variant {
            let _ = write!(url, "variant={}&", variant);
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
            .ok_or(CustomClientError::MissingElement(".ranking-page-table"))?;

        let tbody = Selector::parse("tbody").unwrap();

        let tbody = ranking_page_table
            .select(&tbody)
            .next()
            .ok_or(CustomClientError::MissingElement("tbody"))?;

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
            .ok_or(CustomClientError::RankNode(1))?
            .children()
            .nth(1)
            .ok_or(CustomClientError::RankNode(2))?
            .children()
            .nth(3)
            .ok_or(CustomClientError::RankNode(3))?;

        match node.value() {
            Node::Element(e) => {
                if let Some(id) = e.attr("data-user-id") {
                    Ok(id.parse::<u32>().unwrap())
                } else {
                    Err(CustomClientError::MissingElement("attribute data-user-id"))
                }
            }
            _ => Err(CustomClientError::MissingElement("attribute data-user-id")),
        }
    }
}

pub struct RankLeaderboard<'s> {
    variant: GameModeVariant,
    leaderboard: LeaderboardType<'s>,
}

impl<'s> RankLeaderboard<'s> {
    pub fn score(mode: GameMode) -> Self {
        Self {
            variant: mode.into(),
            leaderboard: LeaderboardType::Score,
        }
    }

    pub fn pp(mode: GameMode, country: Option<&'s str>) -> Self {
        Self {
            variant: mode.into(),
            leaderboard: LeaderboardType::Pp { country },
        }
    }

    pub fn variant_4k(mut self) -> Self {
        if let GameModeVariant::Mania(variant) = &mut self.variant {
            variant.replace(ManiaVariant::K4);
        }

        self
    }

    pub fn variant_7k(mut self) -> Self {
        if let GameModeVariant::Mania(variant) = &mut self.variant {
            variant.replace(ManiaVariant::K7);
        }

        self
    }
}

enum GameModeVariant {
    Osu,
    Taiko,
    Fruits,
    Mania(Option<ManiaVariant>),
}

impl From<GameMode> for GameModeVariant {
    fn from(mode: GameMode) -> Self {
        match mode {
            GameMode::STD => GameModeVariant::Osu,
            GameMode::TKO => GameModeVariant::Taiko,
            GameMode::CTB => GameModeVariant::Fruits,
            GameMode::MNA => GameModeVariant::Mania(None),
        }
    }
}

impl fmt::Display for GameModeVariant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mode = match self {
            Self::Osu => "osu",
            Self::Taiko => "taiko",
            Self::Fruits => "fruits",
            Self::Mania(_) => "mania",
        };

        f.write_str(mode)
    }
}

pub enum ManiaVariant {
    K4,
    K7,
}

impl FromStr for ManiaVariant {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('+') {
            return Err(());
        }

        match s[1..].cow_to_lowercase().as_ref() {
            "4k" | "k4" => Ok(Self::K4),
            "7k" | "k7" => Ok(Self::K7),
            _ => Err(()),
        }
    }
}

impl fmt::Display for ManiaVariant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let variant = match self {
            Self::K4 => "4k",
            Self::K7 => "7k",
        };

        f.write_str(variant)
    }
}

enum LeaderboardType<'s> {
    Score,
    Pp { country: Option<&'s str> },
}

pub enum RankParam {
    Rank(usize),
    Pp(f32),
}
