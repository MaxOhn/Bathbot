use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Write,
};

use bathbot_model::{
    SnipeCountries, SnipeCountryListOrder, SnipeCountryPlayer, SnipeCountryStatistics, SnipePlayer,
    SnipePlayerListOrder, SnipeRecent, SnipeScore, SnipeScoreParams, SnipedPlayer, SnipedWeek,
};
use bathbot_util::IntHasher;
use eyre::Result;
use rosu_v2::model::{GameMode, mods::GameModsIntermode, user::Username};
use time::{Date, Duration, OffsetDateTime};

use crate::Client;

mod huismetbenen;
mod kittenroleplay;

impl Client {
    pub async fn get_snipe_player(
        &self,
        country: &str,
        user_id: u32,
        mode: GameMode,
    ) -> Result<Option<SnipePlayer>> {
        match mode {
            GameMode::Osu => huismetbenen::get_snipe_player(self, country, user_id).await,
            GameMode::Catch | GameMode::Mania => {
                let stats_fut = kittenroleplay::get_snipe_player(self, user_id, mode);
                let mod_counts_fut = kittenroleplay::get_mod_counts(self, user_id, mode);
                let player_stars_fut = kittenroleplay::get_player_stars(self, user_id, mode);

                let params = SnipeScoreParams::new(user_id, country, mode)
                    .limit(1)
                    .order(SnipePlayerListOrder::Date)
                    .descending(false);

                let oldest_score_fut = kittenroleplay::get_national_firsts(self, &params);

                let (stats, mod_counts, player_stars, mut oldest_score) = tokio::try_join!(
                    stats_fut,
                    mod_counts_fut,
                    player_stars_fut,
                    oldest_score_fut,
                )?;

                let Some(stats) = stats else {
                    return Ok(None);
                };

                let mut mods_buf = String::new();

                let count_mods = mod_counts
                    .into_iter()
                    .map(|count| {
                        let mods = GameModsIntermode::from_bits(count.mods);
                        let _ = write!(mods_buf, "{mods}");
                        let mods = Box::from(mods_buf.as_str());
                        mods_buf.clear();

                        (mods, count.count)
                    })
                    .collect();

                let count_sr_spread = player_stars
                    .into_iter()
                    .map(|count| (count.stars as i8, count.count))
                    .collect();

                let Some(oldest_score) = oldest_score.pop() else {
                    return Ok(None);
                };

                let player = SnipePlayer {
                    username: stats.username,
                    user_id,
                    avg_pp: stats.average_pp.unwrap_or(0.0),
                    avg_acc: stats.average_accuracy,
                    avg_stars: stats.average_stars,
                    avg_score: stats.average_score,
                    count_first: stats.count,
                    count_loved: stats.count_loved,
                    count_ranked: stats.count_ranked,
                    difference: stats.count_delta,
                    count_mods,
                    count_sr_spread,
                    oldest_map_id: Some(oldest_score.map_id),
                };

                Ok(Some(player))
            }
            GameMode::Taiko => unimplemented!(),
        }
    }

    pub async fn get_snipe_player_history(
        &self,
        country: &str,
        user_id: u32,
        mode: GameMode,
    ) -> Result<BTreeMap<Date, u32>> {
        match mode {
            GameMode::Osu => huismetbenen::get_snipe_player_history(self, country, user_id).await,
            GameMode::Catch | GameMode::Mania => {
                kittenroleplay::get_snipe_player_history(self, user_id, mode)
                    .await
                    .map(|history| {
                        history
                            .into_iter()
                            .map(|entry| (entry.date.date(), entry.count))
                            .collect()
                    })
            }
            GameMode::Taiko => unimplemented!(),
        }
    }

    pub async fn get_snipe_country(
        &self,
        country_code: &str,
        sort: SnipeCountryListOrder,
        mode: GameMode,
    ) -> Result<Vec<SnipeCountryPlayer>> {
        match mode {
            GameMode::Osu => huismetbenen::get_snipe_country(self, country_code, sort).await,
            GameMode::Catch | GameMode::Mania => {
                let players = kittenroleplay::get_snipe_country(self, country_code, sort, mode)
                    .await?
                    .into_iter()
                    .map(|player| SnipeCountryPlayer {
                        username: player.username,
                        user_id: player.user_id,
                        avg_pp: player.average_pp,
                        avg_sr: player.average_stars,
                        pp: player.weighted_pp.unwrap_or(0.0),
                        count_first: player.count,
                    })
                    .collect();

                Ok(players)
            }
            GameMode::Taiko => unimplemented!(),
        }
    }

    pub async fn get_country_statistics(
        &self,
        country_code: &str,
        mode: GameMode,
    ) -> Result<SnipeCountryStatistics> {
        match mode {
            GameMode::Osu => huismetbenen::get_country_statistics(self, country_code).await,
            GameMode::Catch | GameMode::Mania => {
                kittenroleplay::get_country_statistics(self, country_code, mode)
                    .await
                    .map(From::from)
            }
            GameMode::Taiko => unimplemented!(),
        }
    }

    pub async fn get_sniped_players(
        &self,
        user_id: u32,
        sniper: bool,
        mode: GameMode,
    ) -> Result<Vec<SnipedWeek>> {
        match mode {
            GameMode::Osu => {
                let now = OffsetDateTime::now_utc();
                let since = now - Duration::weeks(8);
                let scores =
                    huismetbenen::get_national_snipes(self, user_id, sniper, since).await?;

                let mut weeks: Vec<_> = (0..8)
                    .filter_map(|weeks| {
                        let until = now - Duration::weeks(weeks);
                        let mut total = 0;
                        let mut unique = HashSet::with_hasher(IntHasher);
                        let mut players = HashMap::with_hasher(IntHasher);

                        if sniper {
                            for score in scores.iter() {
                                let Some(date) = score.date else {
                                    continue;
                                };

                                if date < since || until < date {
                                    continue;
                                }

                                let Some(user_id) = score.sniped_id else {
                                    continue;
                                };

                                total += 1;
                                unique.insert(user_id);

                                let username = score
                                    .sniped
                                    .as_ref()
                                    .map_or_else(|| "<unknown name>".into(), Username::clone);

                                players
                                    .entry(user_id)
                                    .and_modify(|player: &mut SnipedPlayer| player.count += 1)
                                    .or_insert_with(|| SnipedPlayer { username, count: 1 });
                            }
                        } else {
                            for score in scores.iter() {
                                let Some(date) = score.date else {
                                    continue;
                                };

                                if date < since || until < date {
                                    continue;
                                }

                                let user_id = score.sniper_id;

                                total += 1;
                                unique.insert(user_id);

                                let username = score.sniper.as_ref().map_or_else(
                                    || format!("<user {}>", score.sniper_id).into(),
                                    Username::clone,
                                );

                                players
                                    .entry(user_id)
                                    .and_modify(|player: &mut SnipedPlayer| player.count += 1)
                                    .or_insert_with(|| SnipedPlayer { username, count: 1 });
                            }
                        }

                        if players.is_empty() {
                            return None;
                        }

                        Some(SnipedWeek {
                            from: since,
                            until,
                            players: players.into_values().collect(),
                            total,
                            unique: unique.len() as u32,
                        })
                    })
                    .collect();

                // First week is sorted by count; names of all other weeks
                // have to be in the same order as for in first week
                let mut iter = weeks.iter_mut();

                if let Some(first_week) = iter.next() {
                    first_week
                        .players
                        .sort_unstable_by_key(|player| Reverse(player.count));
                    first_week.players.truncate(10);

                    for week in iter {
                        week.players.sort_unstable_by_key(|player| {
                            first_week.players.iter().position(|first_week_player| {
                                first_week_player.username == player.username
                            })
                        });
                    }
                }

                weeks.reverse();
                weeks.dedup_by(|a, b| a.players == b.players);
                weeks.reverse();

                Ok(weeks)
            }
            GameMode::Catch | GameMode::Mania => {
                let mut weeks =
                    kittenroleplay::get_sniped_players(self, user_id, sniper, mode).await?;

                weeks.retain(|week| !week.players.is_empty());
                weeks.reverse();
                weeks.dedup_by(|a, b| a.players == b.players);
                weeks.reverse();

                Ok(weeks)
            }
            GameMode::Taiko => unimplemented!(),
        }
    }

    pub async fn get_national_snipes(
        &self,
        user_id: u32,
        sniper: bool,
        since: OffsetDateTime,
        mode: GameMode,
    ) -> Result<Vec<SnipeRecent>> {
        match mode {
            GameMode::Osu => huismetbenen::get_national_snipes(self, user_id, sniper, since).await,
            GameMode::Catch | GameMode::Mania => {
                let days_since = (OffsetDateTime::now_utc() - since).whole_days() as u32;
                let mut offset = 0;

                let mut scores = Vec::new();

                loop {
                    let new_scores_fut = kittenroleplay::get_national_snipes(
                        self, user_id, sniper, offset, days_since, mode,
                    );

                    let new_scores = new_scores_fut.await?;
                    let new_count = new_scores.len();

                    scores.extend(new_scores.into_iter().map(|snipe| SnipeRecent {
                        map_id: snipe.map_id,
                        user_id,
                        pp: snipe.pp,
                        stars: Some(snipe.stars),
                        accuracy: snipe.accuracy,
                        date: Some(snipe.sniped_at),
                        mods: GameModsIntermode::from_bits(snipe.mods).try_with_mode(mode),
                        max_combo: Some(snipe.max_combo),
                        artist: snipe.artist,
                        title: snipe.title,
                        version: snipe.version,
                        sniper: Some(snipe.sniper_username),
                        sniper_id: snipe.sniper_user_id,
                        sniped: snipe.victim_username,
                        sniped_id: snipe.victim_user_id,
                    }));

                    if new_count < 50 {
                        break;
                    }

                    offset += 50;
                }

                Ok(scores)
            }
            GameMode::Taiko => unimplemented!(),
        }
    }

    pub async fn get_national_firsts(&self, params: &SnipeScoreParams) -> Result<Vec<SnipeScore>> {
        match params.mode {
            GameMode::Osu => huismetbenen::get_national_firsts(self, params).await,
            GameMode::Catch | GameMode::Mania => {
                let scores = kittenroleplay::get_national_firsts(self, params)
                    .await?
                    .into_iter()
                    .map(|score| SnipeScore {
                        score: score.score,
                        pp: score.pp,
                        stars: score.stars,
                        accuracy: score.accuracy,
                        count_miss: Some(score.count_miss),
                        date_set: Some(score.created_at),
                        mods: GameModsIntermode::from_bits(score.mods).try_with_mode(params.mode),
                        max_combo: Some(score.max_combo),
                        map_id: score.map_id,
                    })
                    .collect();

                Ok(scores)
            }
            GameMode::Taiko => unimplemented!(),
        }
    }

    pub async fn get_national_firsts_count(&self, params: &SnipeScoreParams) -> Result<usize> {
        match params.mode {
            GameMode::Osu => huismetbenen::get_national_firsts_count(self, params).await,
            GameMode::Catch | GameMode::Mania => {
                kittenroleplay::get_national_firsts_count(self, params).await
            }
            GameMode::Taiko => unimplemented!(),
        }
    }

    /// Don't use this; use `RedisManager::snipe_countries` instead.
    pub async fn get_snipe_countries(&self, mode: GameMode) -> Result<SnipeCountries> {
        let mut countries = match mode {
            GameMode::Osu => huismetbenen::get_countries(self).await?,
            GameMode::Catch | GameMode::Mania => kittenroleplay::get_countries(self, mode)
                .await
                .map(From::from)?,
            GameMode::Taiko => unimplemented!(),
        };

        countries.sort();

        Ok(countries)
    }
}
