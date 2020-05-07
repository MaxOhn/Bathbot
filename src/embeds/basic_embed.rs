use super::util;
use crate::{
    commands::{
        messages_fun::{MessageActivity, MessageStats},
        utility::AvatarUser,
    },
    scraper::{MostPlayedMap, ScraperScore},
    streams::{TwitchStream, TwitchUser},
    util::{
        datetime::{date_to_string, how_long_ago, sec_to_minsec},
        discord::{self, CacheData},
        globals::{AVATAR_URL, HOMEPAGE, MAP_THUMB_URL, TWITCH_BASE},
        numbers::{round, round_and_comma, round_precision, with_comma_u64},
        osu,
        pp::PPProvider,
        Error,
    },
    MySQL,
};

use itertools::Itertools;
use rayon::prelude::*;
use rosu::models::{
    Beatmap, GameMod, GameMode, GameMods, Grade, Match, Score, Team, TeamType, User,
};
use serenity::{
    builder::CreateEmbed,
    cache::CacheRwLock,
    model::{
        channel::Message,
        gateway::Presence,
        id::{GuildId, RoleId, UserId},
        misc::Mentionable,
    },
    prelude::{RwLock, TypeMap},
    utils::{content_safe, Colour, ContentSafeOptions},
};
use std::{
    cmp::Ordering,
    cmp::Ordering::Equal,
    collections::{BTreeMap, HashMap},
    f32,
    fmt::Write,
    sync::Arc,
    u32,
};

#[derive(Default, Debug)]
pub struct BasicEmbedData {
    pub author_icon: Option<String>,
    pub author_url: Option<String>,
    pub author_text: Option<String>,
    pub title_text: Option<String>,
    pub title_url: Option<String>,
    pub thumbnail: Option<String>,
    pub footer_text: Option<String>,
    pub footer_icon: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub fields: Option<Vec<(String, String, bool)>>,
}

impl BasicEmbedData {
    // ------------------------
    // BUILD THE FINISHED EMBED
    // ------------------------
    pub fn build(self, e: &mut CreateEmbed) -> &mut CreateEmbed {
        if self.author_icon.is_some() || self.author_url.is_some() || self.author_text.is_some() {
            e.author(|a| {
                if let Some(author_icon) = self.author_icon.as_ref() {
                    a.icon_url(author_icon);
                }
                if let Some(author_url) = self.author_url.as_ref() {
                    a.url(author_url);
                }
                if let Some(author_text) = self.author_text.as_ref() {
                    a.name(author_text);
                }
                a
            });
        }
        if self.footer_text.is_some() || self.footer_icon.is_some() {
            e.footer(|f| {
                if let Some(footer_text) = self.footer_text.as_ref() {
                    f.text(footer_text);
                }
                if let Some(footer_icon) = self.footer_icon.as_ref() {
                    f.icon_url(footer_icon);
                }
                f
            });
        }
        if let Some(title) = self.title_text.as_ref() {
            e.title(title);
        }
        if let Some(url) = self.title_url.as_ref() {
            e.url(url);
        }
        if let Some(thumbnail) = self.thumbnail.as_ref() {
            e.thumbnail(thumbnail);
        }
        if let Some(description) = self.description.as_ref() {
            e.description(description);
        }
        if let Some(image_url) = self.image_url.as_ref() {
            e.image(image_url);
        }
        if let Some(fields) = self.fields {
            e.fields(fields);
        }
        e.color(Colour::DARK_GREEN)
    }

    //
    // activity
    //
    pub fn create_activity(activity: MessageActivity, name: String, channel: bool) -> Self {
        let mut result = Self::default();
        let title = format!(
            "Message activity in {} {}{}:",
            if channel { "channel" } else { "server" },
            if channel { "#" } else { "" },
            name
        );
        let month_str = with_comma_u64(activity.hour as u64);
        let amount_len = 9.max(month_str.len());
        let mut description = String::with_capacity(128);
        let _ = writeln!(description, "```");
        let _ = writeln!(
            description,
            " Last | {:>len$} |   Total | Activity",
            "#Messages",
            len = amount_len
        );
        let _ = writeln!(
            description,
            "------+-{:->len$}-+---------+---------",
            "-",
            len = amount_len
        );
        let activity_hour = round((100 * 24 * 30 * activity.hour) as f32 / activity.month as f32);
        let activity_day = round((100 * 30 * activity.day) as f32 / activity.month as f32);
        let activity_week = round(100.0 * 4.286 * activity.week as f32 / activity.month as f32);
        let _ = writeln!(
            description,
            " Hour | {:>len$} | {:>6}% | {:>7}%",
            with_comma_u64(activity.hour as u64),
            round(100.0 * activity.hour as f32 / activity.month as f32),
            activity_hour,
            len = amount_len
        );
        let _ = writeln!(
            description,
            "  Day | {:>len$} | {:>6}% | {:>7}%",
            with_comma_u64(activity.day as u64),
            round(100.0 * activity.day as f32 / activity.month as f32),
            activity_day,
            len = amount_len
        );
        let _ = writeln!(
            description,
            " Week | {:>len$} | {:>6}% | {:>7}%",
            with_comma_u64(activity.week as u64),
            round(100.0 * activity.week as f32 / activity.month as f32),
            activity_week,
            len = amount_len
        );
        let _ = writeln!(
            description,
            "Month | {:>len$} | {:>6}% | {:>7}%",
            with_comma_u64(activity.month as u64),
            100,
            100,
            len = amount_len
        );
        let _ = write!(description, "```");
        result.title_text = Some(title);
        result.description = Some(description);
        result
    }

    //
    // allstreams
    //
    pub fn create_allstreams(
        presences: Vec<Presence>,
        users: HashMap<UserId, String>,
        total: usize,
        thumbnail: Option<String>,
    ) -> Self {
        let mut result = Self::default();
        result.thumbnail = thumbnail;
        result.title_text = Some(format!("{} current streamers on this server:", total));
        // No streamers -> Simple message
        let description = if presences.is_empty() {
            "No one is currently streaming".to_string()
        // Less than 20 streamers -> Descriptive single column
        } else if presences.len() <= 20 {
            let mut description = String::with_capacity(512);
            for p in presences {
                let activity = p.activity.expect("activity");
                let _ = write!(
                    description,
                    "- {name} playing {game}",
                    name = users.get(&p.user_id).unwrap(),
                    game = activity
                        .state
                        .unwrap_or_else(|| panic!("Could not get state of activity"))
                );
                if let Some(title) = activity.details {
                    if let Some(url) = activity.url {
                        let _ = writeln!(description, ": [{}]({})", title, url);
                    } else {
                        let _ = writeln!(description, ": {}", title);
                    }
                } else {
                    description.push('\n');
                }
            }
            description
        // Less than 40 streamers -> Two simple columns
        } else if presences.len() <= 40 {
            let mut description = String::with_capacity(768);
            for mut chunk in presences.into_iter().chunks(2).into_iter() {
                // First
                let first: Presence = chunk.next().unwrap();
                let activity = first.activity.unwrap();
                let _ = write!(
                    description,
                    "- {name}: ",
                    name = users.get(&first.user_id).unwrap(),
                );
                let game = activity
                    .state
                    .unwrap_or_else(|| panic!("Could not get state of activity"));
                if let Some(url) = activity.url {
                    let _ = write!(description, "[{}]({})", game, url);
                } else {
                    description.push_str(&game);
                }
                // Second
                if let Some(second) = chunk.next() {
                    let _ = write!(
                        description,
                        " ~ {name}: ",
                        name = users.get(&second.user_id).unwrap()
                    );
                    let activity = second.activity.unwrap();
                    let game = activity
                        .state
                        .unwrap_or_else(|| panic!("Could not get state of activity"));
                    if let Some(url) = activity.url {
                        let _ = write!(description, "[{}]({})", game, url);
                    } else {
                        description.push_str(&game);
                    }
                    description.push('\n');
                }
            }
            description
        // 40+ Streamers -> Three simple columns
        } else {
            if presences.len() == 60 {
                result.title_text = Some(format!(
                    "60 out of {} current streamers of this server:",
                    total
                ));
            }
            let mut description = String::with_capacity(1024);
            for mut chunk in presences.into_iter().chunks(3).into_iter() {
                // First
                let first: Presence = chunk.next().unwrap();
                let activity = first.activity.unwrap();
                let _ = write!(
                    description,
                    "- {name}: ",
                    name = users.get(&first.user_id).unwrap(),
                );
                let game = activity
                    .state
                    .unwrap_or_else(|| panic!("Could not get state of activity"));
                if let Some(url) = activity.url {
                    let _ = write!(description, "[{}]({})", game, url);
                } else {
                    description.push_str(&game);
                }
                // Second
                if let Some(second) = chunk.next() {
                    let _ = write!(
                        description,
                        " ~ {name}: ",
                        name = users.get(&second.user_id).unwrap()
                    );
                    let activity = second.activity.unwrap();
                    let game = activity
                        .state
                        .unwrap_or_else(|| panic!("Could not get state of activity"));
                    if let Some(url) = activity.url {
                        let _ = write!(description, "[{}]({})", game, url);
                    } else {
                        description.push_str(&game);
                    }
                    // Third
                    if let Some(third) = chunk.next() {
                        let _ = write!(
                            description,
                            " ~ {name}: ",
                            name = users.get(&third.user_id).unwrap()
                        );
                        let activity = third.activity.unwrap();
                        let game = activity
                            .state
                            .unwrap_or_else(|| panic!("Could not get state of activity"));
                        if let Some(url) = activity.url {
                            let _ = write!(description, "[{}]({})", game, url);
                        } else {
                            description.push_str(&game);
                        }
                        description.push('\n');
                    }
                }
            }
            description
        };
        result.description = Some(description);
        result
    }

    //
    // bg ranking
    //
    pub fn create_bg_ranking(
        author_idx: Option<usize>,
        list: Vec<(&String, u32)>,
        global: bool,
        idx: usize,
        pages: (usize, usize),
    ) -> Self {
        let symbols = ["♔", "♕", "♖", "♗", "♘", "♙"];
        let mut result = Self::default();
        let len = list.iter().fold(0, |max, (user, _)| max.max(user.len()));
        let mut description = String::with_capacity(256);
        description.push_str("```\n");
        for (mut i, (user, score)) in list.into_iter().enumerate() {
            i += idx;
            let _ = writeln!(
                description,
                "{:>2} {:1} # {:<len$} => {}",
                i,
                if i < 7 {
                    symbols.get(i - 1).unwrap()
                } else {
                    ""
                },
                user,
                score,
                len = len
            );
        }
        description.push_str("```");
        let mut footer_text = format!("Page {}/{}", pages.0, pages.1);
        if let Some(author_idx) = author_idx {
            let _ = write!(footer_text, " ~ Your rank: {}", author_idx + 1);
        }
        let author_text = format!(
            "{} leaderboard for correct guesses:",
            if global { "Global" } else { "Server" }
        );
        result.footer_text = Some(footer_text);
        result.description = Some(description);
        result.author_text = Some(author_text);
        result
    }

    //
    // command counter
    //
    pub fn create_command_counter(
        list: Vec<(&String, u32)>,
        booted_up: &str,
        idx: usize,
        pages: (usize, usize),
    ) -> Self {
        let symbols = ["♔", "♕", "♖", "♗", "♘", "♙"];
        let mut result = Self::default();
        let len = list.iter().fold(0, |max, (name, _)| max.max(name.len()));
        let mut description = String::with_capacity(256);
        description.push_str("```\n");
        for (mut i, (name, amount)) in list.into_iter().enumerate() {
            i += idx;
            let _ = writeln!(
                description,
                "{:>2} {:1} # {:<len$} => {}",
                i,
                if i < 7 {
                    symbols.get(i - 1).unwrap()
                } else {
                    ""
                },
                name,
                amount,
                len = len
            );
        }
        description.push_str("```");
        let footer_text = format!(
            "Page {}/{} ~ Started counting {}",
            pages.0, pages.1, booted_up
        );
        result.description = Some(description);
        result.footer_text = Some(footer_text);
        result.author_text = Some("Most popular commands:".to_string());
        result
    }

    //
    // avatar
    //
    pub fn create_avatar(user: AvatarUser) -> Self {
        let mut result = Self::default();
        let title_text = format!(
            "{}'s {} avatar:",
            user.name(),
            if let AvatarUser::Discord { .. } = user {
                "discord"
            } else {
                "osu!"
            }
        );
        result.title_text = Some(title_text);
        result.title_url = Some(user.url().to_string());
        result.image_url = Some(user.url().to_string());
        result
    }

    //
    // common
    //
    /// Returns a tuple containing a new `BasicEmbedData` object,
    /// and a `Vec<u8>` representing the bytes of a png
    pub async fn create_common(
        users: HashMap<u32, User>,
        all_scores: Vec<Vec<Score>>,
        maps: HashMap<u32, Beatmap>,
    ) -> (Self, Vec<u8>) {
        let mut result = Self::default();
        // Flatten scores, sort by beatmap id, then group by beatmap id
        let mut all_scores: Vec<Score> = all_scores.into_iter().flatten().collect();
        all_scores.sort_by(|s1, s2| s1.beatmap_id.unwrap().cmp(&s2.beatmap_id.unwrap()));
        let mut all_scores: HashMap<u32, Vec<Score>> = all_scores
            .into_iter()
            .group_by(|score| score.beatmap_id.unwrap())
            .into_iter()
            .map(|(map_id, scores)| (map_id, scores.collect()))
            .collect();
        // Sort each group by pp value, then take the best 3
        all_scores.par_iter_mut().for_each(|(_, scores)| {
            scores.sort_by(|s1, s2| s2.pp.unwrap().partial_cmp(&s1.pp.unwrap()).unwrap());
            scores.truncate(3);
        });
        // Consider only the top 10 maps with the highest avg pp among the users
        let mut pp_avg: Vec<(u32, f32)> = all_scores
            .par_iter()
            .map(|(&map_id, scores)| {
                let sum = scores.iter().fold(0.0, |sum, next| sum + next.pp.unwrap());
                (map_id, sum / scores.len() as f32)
            })
            .collect();
        pp_avg.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top_map_ids: Vec<u32> = pp_avg.into_iter().take(10).map(|(id, _)| id).collect();
        all_scores.retain(|id, _| top_map_ids.contains(id));
        // Write msg
        let mut description = String::with_capacity(512);
        for (i, map_id) in top_map_ids.iter().enumerate() {
            let map = maps.get(map_id).unwrap();
            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id})",
                idx = i + 1,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
            );
            let scores = all_scores.get(map_id).unwrap();
            let first_score = scores.get(0).unwrap();
            let first_user = users.get(&first_score.user_id).unwrap();
            let second_score = scores.get(1).unwrap();
            let second_user = users.get(&second_score.user_id).unwrap();
            let _ = write!(
                description,
                "- :first_place: `{}`: {}pp :second_place: `{}`: {}pp",
                first_user.username,
                round(first_score.pp.unwrap()),
                second_user.username,
                round(second_score.pp.unwrap())
            );
            if users.len() > 2 {
                let third_score = scores.get(2).unwrap();
                let third_user = users.get(&third_score.user_id).unwrap();
                let _ = write!(
                    description,
                    " :third_place: `{}`: {}pp",
                    third_user.username,
                    round(third_score.pp.unwrap())
                );
            }
            description.push('\n');
        }
        // Keys have no strict order, hence inconsistent result
        let user_ids: Vec<u32> = users.keys().copied().collect();
        let thumbnail = discord::get_combined_thumbnail(&user_ids)
            .await
            .unwrap_or_else(|e| {
                warn!("Error while combining avatars: {}", e);
                Vec::default()
            });
        result.description = Some(description);
        (result, thumbnail)
    }

    //
    // leaderboard
    //
    pub async fn create_leaderboard<'i, S, D>(
        init_name: &Option<&str>,
        map: &Beatmap,
        scores: Option<S>,
        author_icon: &Option<String>,
        idx: usize,
        cache_data: D,
    ) -> Result<Self, Error>
    where
        S: Iterator<Item = &'i ScraperScore>,
        D: CacheData,
    {
        let mut result = Self::default();
        let mut author_text = String::with_capacity(16);
        if map.mode == GameMode::MNA {
            let _ = write!(author_text, "[{}K] ", map.diff_cs as u32);
        }
        let _ = write!(author_text, "{} [{}★]", map, round(map.stars));
        let author_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id);
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        let description = if let Some(scores) = scores {
            let mut mod_map = HashMap::new();
            let mut description = String::with_capacity(256);
            let author_name = init_name.map_or_else(String::new, |n| n.to_lowercase());
            for (i, score) in scores.enumerate() {
                let found_author = author_name == score.username.to_lowercase();
                let mut username = String::with_capacity(32);
                if found_author {
                    username.push_str("__");
                }
                let _ = write!(
                    username,
                    "[{name}](https://osu.ppy.sh/users/{id})",
                    name = score.username,
                    id = score.user_id
                );
                if found_author {
                    username.push_str("__");
                }
                let cache = cache_data.cache().clone();
                let data = Arc::clone(cache_data.data());
                let _ = writeln!(
                    description,
                    "**{idx}.** {emote} **{name}**: {score} [ {combo} ]{mods}\n\
                    - {pp} ~ {acc}% ~ {ago}",
                    idx = idx + i + 1,
                    emote = osu::grade_emote(score.grade, cache).await.to_string(),
                    name = username,
                    score = with_comma_u64(score.score as u64),
                    combo = get_combo(&score, &map),
                    mods = if score.enabled_mods.is_empty() {
                        String::new()
                    } else {
                        format!(" **+{}**", score.enabled_mods)
                    },
                    pp = get_pp(&mut mod_map, &score, &map, data).await?,
                    acc = round(score.accuracy),
                    ago = how_long_ago(&score.date),
                );
            }
            description
        } else {
            "No scores found".to_string()
        };
        result.thumbnail = Some(thumbnail);
        result.author_icon = author_icon.clone();
        result.author_text = Some(author_text);
        result.author_url = Some(author_url);
        result.description = Some(description);
        result.footer_text = Some(footer_text);
        result.footer_icon = Some(footer_url);
        Ok(result)
    }

    //
    //  matchcosts
    //
    pub fn create_match_costs(
        mut users: HashMap<u32, String>,
        osu_match: Match,
        warmups: usize,
    ) -> Self {
        let mut result = Self::default();
        let mut thumbnail = None;
        let title_url = format!("{}community/matches/{}", HOMEPAGE, osu_match.match_id);
        let mut title_text = osu_match.name;
        title_text.retain(|c| c != '(' && c != ')');
        let description = if osu_match.games.len() <= warmups {
            let mut description = String::from("No games played yet");
            if !osu_match.games.is_empty() && warmups > 0 {
                let _ = write!(
                    description,
                    " beyond the {} warmup{}",
                    warmups,
                    if warmups > 1 { "s" } else { "" }
                );
            }
            description
        } else {
            let games: Vec<_> = osu_match.games.into_iter().skip(warmups).collect();
            let games_len = games.len() as f32;
            let mut teams = HashMap::new();
            let mut point_costs = HashMap::new();
            let team_vs = games.first().unwrap().team_type == TeamType::TeamVS;
            for mut game in games {
                game.scores = game.scores.into_iter().filter(|s| s.score > 0).collect();
                let score_sum: u32 = game.scores.iter().map(|s| s.score).sum();
                let avg = score_sum as f32 / game.scores.len() as f32;
                for score in game.scores {
                    let point_cost = score.score as f32 / avg + 0.4;
                    point_costs
                        .entry(score.user_id)
                        .or_insert_with(Vec::new)
                        .push(point_cost);
                    teams.entry(score.user_id).or_insert(score.team);
                }
            }
            let mut data = HashMap::new();
            let mut highest_cost = 0.0;
            let mut mvp_id = 0;
            for (user, point_costs) in point_costs {
                let name = users.remove(&user).unwrap();
                let sum: f32 = point_costs.iter().sum();
                let costs_len = point_costs.len() as f32;
                let mut match_cost = sum / costs_len;
                match_cost *= 1.2_f32.powf((costs_len / games_len).powf(0.4));
                data.entry(*teams.get(&user).unwrap())
                    .or_insert_with(Vec::new)
                    .push((name, match_cost));
                if match_cost > highest_cost {
                    highest_cost = match_cost;
                    mvp_id = user;
                }
            }
            thumbnail = Some(format!("{}{}", AVATAR_URL, mvp_id));
            let player_comparer =
                |a: &(String, f32), b: &(String, f32)| b.1.partial_cmp(&a.1).unwrap();
            let mut description = String::with_capacity(256);
            if team_vs {
                let blue = match data.remove(&Team::Blue) {
                    Some(mut team) => {
                        team.sort_by(player_comparer);
                        team
                    }
                    None => Vec::new(),
                };
                let red = match data.remove(&Team::Red) {
                    Some(mut team) => {
                        team.sort_by(player_comparer);
                        team
                    }
                    None => Vec::new(),
                };
                let blue_len = blue.len();
                let blue_has_mvp = blue_len > 0
                    && (red.is_empty() || blue.first().unwrap().1 > red.first().unwrap().1);
                if blue_len > 0 {
                    let _ = writeln!(description, ":blue_circle: **Blue Team** :blue_circle:");
                    add_team(&mut description, blue, blue_has_mvp);
                }
                if !red.is_empty() {
                    if blue_len > 0 {
                        description.push('\n');
                    }
                    let _ = writeln!(description, ":red_circle: **Red Team** :red_circle:");
                    add_team(&mut description, red, !blue_has_mvp);
                }
                description
            } else {
                let mut players = data.remove(&Team::None).unwrap_or_default();
                players.sort_by(player_comparer);
                add_team(&mut description, players, true);
                description
            }
        };
        result.title_text = Some(title_text);
        result.title_url = Some(title_url);
        result.thumbnail = thumbnail;
        result.description = Some(description);
        result
    }

    //
    // messagestats
    //
    pub fn create_messagestats(stats: MessageStats, author: &str) -> Self {
        let mut result = Self::default();
        let description = format!(
            "All saved messages: {}\n\
            Messages of this guild: {}\n\
            Messages of this channel: {}",
            with_comma_u64(stats.total_msgs as u64),
            with_comma_u64(stats.guild_msgs as u64),
            with_comma_u64(stats.channel_msgs as u64)
        );
        let single_words: Vec<_> = stats
            .single_words
            .into_iter()
            .map(|(word, amount)| (word, with_comma_u64(amount as u64)))
            .collect();
        let max_single_word = single_words
            .iter()
            .map(|(_, amount)| amount.len())
            .max()
            .unwrap();
        let field_text = single_words
            .into_iter()
            .map(|(word, amount)| {
                format!(
                    "{:<max$} => {word}",
                    amount = amount,
                    max = max_single_word,
                    word = word,
                )
            })
            .join("\n");
        let fields = vec![
            (
                format!("Messages of `{}`", author),
                with_comma_u64(stats.author_msgs as u64),
                false,
            ),
            (
                format!("Average length of `{}`'s messages", author),
                round(stats.author_avg).to_string(),
                false,
            ),
            (
                format!("`{}`'s most common single word messages", author),
                format!("```\n{}```", field_text),
                false,
            ),
        ];
        result.title_text = Some("Statistics about the message database".to_string());
        result.description = Some(description);
        result.fields = Some(fields);
        result
    }

    //
    // mostplayed
    //
    pub fn create_mostplayed<'m, M>(user: &User, maps: M, pages: (usize, usize)) -> Self
    where
        M: Iterator<Item = &'m MostPlayedMap>,
    {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(10 * 70);
        for map in maps {
            let _ = writeln!(
                description,
                "**[{count}]** [{artist} - {title} [{version}]]({base}b/{map_id}) [{stars}]",
                count = map.count,
                title = map.title,
                artist = map.artist,
                version = map.version,
                base = HOMEPAGE,
                map_id = map.beatmap_id,
                stars = util::get_stars(map.stars),
            );
        }
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.title_text = Some("Most played maps:".to_string());
        result.thumbnail = Some(thumbnail);
        result.description = Some(description);
        result.footer_text = Some(format!("Page {}/{}", pages.0, pages.1));
        result
    }

    //
    // mostplayedcommon
    //
    /// Returns a tuple containing a new `BasicEmbedData` object,
    /// and a `Vec<u8>` representing the bytes of a png
    pub async fn create_mostplayedcommon(
        users: HashMap<u32, User>,
        mut maps: Vec<MostPlayedMap>,
        users_count: HashMap<u32, HashMap<u32, u32>>,
    ) -> (Self, Vec<u8>) {
        let mut result = Self::default();
        // Sort maps by sum of counts
        let total_counts: HashMap<u32, u32> = users_count.iter().fold(
            HashMap::with_capacity(maps.len()),
            |mut counts, (_, user_entry)| {
                for (map_id, count) in user_entry {
                    *counts.entry(*map_id).or_insert(0) += count;
                }
                counts
            },
        );
        maps.sort_by(|a, b| {
            total_counts
                .get(&b.beatmap_id)
                .unwrap()
                .cmp(total_counts.get(&a.beatmap_id).unwrap())
        });
        // Write msg
        let mut description = String::with_capacity(512);
        for (i, map) in maps.into_iter().enumerate() {
            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id}) [{stars}]",
                idx = i + 1,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
                stars = util::get_stars(map.stars),
            );
            let mut top_users: Vec<(u32, u32)> = users_count
                .iter()
                .map(|(user_id, entry)| (*user_id, *entry.get(&map.beatmap_id).unwrap()))
                .collect();
            top_users.sort_by(|a, b| b.1.cmp(&a.1));
            let mut top_users = top_users.into_iter().take(3);
            let (first_name, first_count) = top_users
                .next()
                .map(|(user_id, count)| (&users.get(&user_id).unwrap().username, count))
                .unwrap();
            let (second_name, second_count) = top_users
                .next()
                .map(|(user_id, count)| (&users.get(&user_id).unwrap().username, count))
                .unwrap();
            let _ = write!(
                description,
                "- :first_place: `{}`: **{}** :second_place: `{}`: **{}**",
                first_name, first_count, second_name, second_count
            );
            if let Some((third_id, third_count)) = top_users.next() {
                let third_name = &users.get(&third_id).unwrap().username;
                let _ = write!(
                    description,
                    " :third_place: `{}`: **{}**",
                    third_name, third_count
                );
            }
            description.push('\n');
        }
        // Keys have no strict order, hence inconsistent result
        let user_ids: Vec<u32> = users.keys().copied().collect();
        let thumbnail = discord::get_combined_thumbnail(&user_ids)
            .await
            .unwrap_or_else(|e| {
                warn!("Error while combining avatars: {}", e);
                Vec::default()
            });
        result.description = Some(description);
        (result, thumbnail)
    }

    //
    // nochoke
    //
    pub async fn create_nochoke(
        user: User,
        scores_data: HashMap<usize, (Score, Beatmap)>,
        cache: CacheRwLock,
    ) -> Result<Self, Error> {
        let mut result = Self::default();
        // 5 would be sufficient but 10 reduces error probability
        let mut index_10_pp: f32 = 0.0; // pp of 10th best unchoked score

        // BTreeMap to keep entries sorted by key
        let mut unchoked_scores: BTreeMap<F32T, (usize, Score)> = BTreeMap::new();
        for (idx, (score, map)) in scores_data.iter() {
            let combo_ratio = score.max_combo as f32 / map.max_combo.unwrap() as f32;
            // If the score is an (almost) fc but already has too few pp, skip
            if combo_ratio > 0.98 && score.pp.unwrap() < index_10_pp * 0.94 {
                continue;
            }
            let mut unchoked = score.clone();
            // If combo isn't max, unchoke the score
            if score.max_combo != map.max_combo.unwrap() {
                osu::unchoke_score(&mut unchoked, map);
                let pp = PPProvider::calculate_oppai_pp(&unchoked, &map).await?;
                unchoked.pp = Some(pp);
            }
            let pp = unchoked.pp.unwrap();
            if pp > index_10_pp {
                unchoked_scores.insert(F32T::new(pp), (*idx, unchoked));
                index_10_pp = unchoked_scores
                    .iter()
                    .rev() // BTreeMap stores entries in ascending order wrt. the key
                    .take(10)
                    .last() // Get 10th entry
                    .unwrap()
                    .0 // Get the entry's key
                    .to_f32(); // F32T to f32
            }
        }
        let unchoked_scores: Vec<(usize, Score, &Score, &Beatmap)> = unchoked_scores
            .into_iter()
            .rev()
            .take(5)
            .map(|(_, (i, unchoked_score))| {
                let (actual_score, map) = scores_data.get(&i).unwrap();
                (i, unchoked_score, actual_score, map)
            })
            .collect();

        // Done calculating, now preparing strings for message
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(512);
        for (idx, unchoked, actual, map) in unchoked_scores.into_iter() {
            let (stars, max_pp) = {
                let pp_provider = PPProvider::new(actual, map, None).await.map_err(|why| {
                    Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    ))
                })?;
                (
                    util::get_stars(pp_provider.stars()),
                    round(pp_provider.max_pp()),
                )
            };
            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {old_pp} → **{new_pp}pp**/{max_pp}PP ~ ({old_acc} → **{new_acc}%**)\n\
                [ {old_combo} → **{new_combo}x**/{max_combo}x ] ~ *Removed {misses} miss{plural}*",
                idx = idx,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
                mods = util::get_mods(&actual.enabled_mods),
                stars = stars,
                grade = osu::grade_emote(unchoked.grade, cache.clone()).await,
                old_pp = round(actual.pp.unwrap()),
                new_pp = round(unchoked.pp.unwrap()),
                max_pp = max_pp,
                old_acc = round(actual.accuracy(GameMode::STD)),
                new_acc = round(unchoked.accuracy(GameMode::STD)),
                old_combo = actual.max_combo,
                new_combo = unchoked.max_combo,
                max_combo = map.max_combo.unwrap(),
                misses = actual.count_miss - unchoked.count_miss,
                plural = if actual.count_miss - unchoked.count_miss != 1 {
                    "es"
                } else {
                    ""
                }
            );
        }
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.description = Some(description);
        Ok(result)
    }

    //
    // pp missing
    //
    pub fn create_ppmissing(user: User, scores: Vec<Score>, pp: f32) -> Self {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let title = format!(
            "What score is {name} missing to reach {pp_given}pp?",
            name = user.username,
            pp_given = pp
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let description = if user.pp_raw > pp {
            format!(
                "{name} already has {pp_raw}pp which is more than {pp_given}pp.\n\
                 No more scores are required.",
                name = user.username,
                pp_raw = round_and_comma(user.pp_raw),
                pp_given = pp
            )
        } else {
            let pp_values: Vec<f32> = scores.into_iter().map(|score| score.pp.unwrap()).collect();
            let size: usize = pp_values.len();
            let mut idx: usize = size - 1;
            let mut factor: f32 = 0.95_f32.powi(idx as i32);
            let mut top: f32 = user.pp_raw;
            let mut bot: f32 = 0.0;
            let mut current: f32 = pp_values[idx];
            while top + bot < pp {
                top -= current * factor;
                if idx == 0 {
                    break;
                }
                current = pp_values[idx - 1];
                bot += current * factor;
                factor /= 0.95;
                idx -= 1;
            }
            let mut required: f32 = pp - top - bot;
            if top + bot >= pp {
                factor *= 0.95;
                required = (required + factor * pp_values[idx]) / factor;
                idx += 1;
            }
            idx += 1;
            if size < 100 {
                required -= pp_values[size - 1] * 0.95_f32.powi(size as i32 - 1);
            }
            format!(
                "To reach {pp}pp with one additional score, {user} needs to perform \
                 a **{required}pp** score which would be the top #{idx}",
                pp = round(pp),
                user = user.username,
                required = round(required),
                idx = idx
            )
        };
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.title_text = Some(title);
        result.description = Some(description);
        result
    }

    //
    // profile
    //
    pub async fn create_profile(
        user: User,
        score_maps: Vec<(Score, Beatmap)>,
        mode: GameMode,
        cache: CacheRwLock,
    ) -> Self {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let footer_text = format!(
            "Joined osu! {} ({})",
            date_to_string(&user.join_date),
            how_long_ago(&user.join_date),
        );
        let bonus_pow = 0.9994_f64.powi(
            (user.count_ssh + user.count_ss + user.count_sh + user.count_s + user.count_a) as i32,
        );
        let bonus_pp = (100.0 * 416.6667 * (1.0 - bonus_pow)).round() / 100.0;
        let mut fields = vec![
            (
                "Ranked score:".to_owned(),
                with_comma_u64(user.ranked_score),
                true,
            ),
            (
                "Total score:".to_owned(),
                with_comma_u64(user.total_score),
                true,
            ),
            (
                "Total hits:".to_owned(),
                with_comma_u64(user.total_hits()),
                true,
            ),
            (
                "Play count / time:".to_owned(),
                format!(
                    "{} / {} hrs",
                    with_comma_u64(user.playcount as u64),
                    user.total_seconds_played / 3600
                ),
                true,
            ),
            ("Level:".to_owned(), round(user.level).to_string(), true),
            ("Bonus PP:".to_owned(), format!("~{}pp", bonus_pp), true),
            (
                "Accuracy:".to_owned(),
                format!("{}%", round(user.accuracy)),
                true,
            ),
        ];
        if score_maps.is_empty() {
            result.description = Some("No Top scores".to_string());
        } else {
            let values = ProfileResult::calc(mode, score_maps);
            let mut combo = String::from(&values.avg_combo.to_string());
            match mode {
                GameMode::STD | GameMode::CTB => {
                    let _ = write!(combo, "/{}", values.map_combo);
                }
                _ => {}
            }
            let _ = write!(combo, " [{} - {}]", values.min_combo, values.max_combo);
            fields.extend(vec![
                (
                    "Unweighted accuracy:".to_owned(),
                    format!(
                        "{}% [{}% - {}%]",
                        round(values.avg_acc),
                        round(values.min_acc),
                        round(values.max_acc)
                    ),
                    true,
                ),
                (
                    "Grades:".to_owned(),
                    format!(
                        "{}{} {}{} {}{} {}{} {}{}",
                        osu::grade_emote(Grade::XH, cache.clone()).await,
                        user.count_ssh,
                        osu::grade_emote(Grade::X, cache.clone()).await,
                        user.count_ss,
                        osu::grade_emote(Grade::SH, cache.clone()).await,
                        user.count_sh,
                        osu::grade_emote(Grade::S, cache.clone()).await,
                        user.count_s,
                        osu::grade_emote(Grade::A, cache).await,
                        user.count_a,
                    ),
                    false,
                ),
                (
                    "Average PP:".to_owned(),
                    format!(
                        "{}pp [{} - {}]",
                        round(values.avg_pp),
                        round(values.min_pp),
                        round(values.max_pp)
                    ),
                    true,
                ),
                ("Average Combo:".to_owned(), combo, true),
            ]);
            if let Some(mod_combs_count) = values.mod_combs_count {
                fields.push((
                    "Favourite mod combinations:".to_owned(),
                    mod_combs_count
                        .into_iter()
                        .map(|(mods, count)| format!("`{} {}%`", mods, count))
                        .join(" > "),
                    false,
                ));
            }
            fields.push((
                "Favourite mods:".to_owned(),
                values
                    .mods_count
                    .into_iter()
                    .map(|(mods, count)| format!("`{} {}%`", mods, count))
                    .join(" > "),
                false,
            ));
            if let Some(mod_combs_pp) = values.mod_combs_pp {
                fields.push((
                    "PP earned with mod combination:".to_owned(),
                    mod_combs_pp
                        .into_iter()
                        .map(|(mods, pp)| format!("`{} {}pp`", mods, round(pp)))
                        .join(" > "),
                    false,
                ));
            }
            fields.push((
                "PP earned with mod:".to_owned(),
                values
                    .mods_pp
                    .into_iter()
                    .map(|(mods, pp)| format!("`{} {}pp`", mods, round(pp)))
                    .join(" > "),
                false,
            ));
            fields.push((
                "Mappers in top 100:".to_owned(),
                values
                    .mappers
                    .into_iter()
                    .map(|(name, count, pp)| format!("{}: {}pp ({})", name, round(pp), count))
                    .join("\n"),
                true,
            ));
            fields.push((
                "Average map length:".to_owned(),
                format!(
                    "{} [{} - {}]",
                    sec_to_minsec(values.avg_len),
                    sec_to_minsec(values.min_len),
                    sec_to_minsec(values.max_len)
                ),
                true,
            ));
        }
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.footer_text = Some(footer_text);
        result.fields = Some(fields);
        result
    }

    //
    // rank
    //
    pub fn create_rank(
        user: User,
        scores: Option<Vec<Score>>,
        rank: usize,
        country: Option<String>,
        rank_holder: User,
    ) -> Self {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let country = country.unwrap_or_else(|| '#'.to_string());
        let title = format!(
            "How many pp is {name} missing to reach rank {country}{rank}?",
            name = user.username,
            country = country,
            rank = rank
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let description = if user.pp_raw > rank_holder.pp_raw {
            format!(
                "Rank {country}{rank} is currently held by {holder_name} with \
                 **{holder_pp}pp**, so {name} is with **{pp}pp** already above that.",
                country = country,
                rank = rank,
                holder_name = rank_holder.username,
                holder_pp = round_and_comma(rank_holder.pp_raw),
                name = user.username,
                pp = round_and_comma(user.pp_raw)
            )
        } else {
            let pp_values: Vec<f32> = scores
                .unwrap_or_else(|| panic!("Got None for scores in create_rank"))
                .into_iter()
                .map(|score| score.pp.unwrap())
                .collect();
            let size: usize = pp_values.len();
            let mut idx: usize = size - 1;
            let mut factor: f32 = 0.95_f32.powi(idx as i32);
            let mut top: f32 = user.pp_raw;
            let mut bot: f32 = 0.0;
            let mut current: f32 = pp_values[idx];
            while top + bot < rank_holder.pp_raw {
                top -= current * factor;
                if idx == 0 {
                    break;
                }
                current = pp_values[idx - 1];
                bot += current * factor;
                factor /= 0.95;
                idx -= 1;
            }
            let mut required: f32 = rank_holder.pp_raw - top - bot;
            if top + bot >= rank_holder.pp_raw {
                factor *= 0.95;
                required = (required + factor * pp_values[idx]) / factor;
            }
            if size < 100 {
                required -= pp_values[size - 1] * 0.95_f32.powi(size as i32 - 1);
            }
            format!(
                "Rank {country}{rank} is currently held by {holder_name} with \
                 **{holder_pp}pp**, so {name} is missing **{missing}** raw pp, \
                 achievable by a single score worth **{pp}pp**.",
                country = country,
                rank = rank,
                holder_name = rank_holder.username,
                holder_pp = round_and_comma(rank_holder.pp_raw),
                name = user.username,
                missing = round_and_comma(rank_holder.pp_raw - user.pp_raw),
                pp = round_and_comma(required),
            )
        };
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.title_text = Some(title);
        result.description = Some(description);
        result
    }

    //
    // ratio
    //
    pub async fn create_ratio(
        user: User,
        scores: Vec<Score>,
        data: Arc<RwLock<TypeMap>>,
    ) -> Result<Self, Error> {
        let mut result = Self::default();
        let mut accs = vec![0, 90, 95, 97, 99];
        let mut categories: BTreeMap<u8, RatioCategory> = BTreeMap::new();
        for &acc in accs.iter() {
            categories.insert(acc, RatioCategory::default());
        }
        categories.insert(100, RatioCategory::default());
        for score in scores {
            let acc = score.accuracy(GameMode::MNA);
            for &curr in accs.iter() {
                if acc > curr as f32 {
                    categories.get_mut(&curr).unwrap().add_score(&score);
                }
            }
            if score.grade.eq_letter(Grade::X) {
                categories.get_mut(&100).unwrap().add_score(&score);
            }
        }
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(256);
        let _ = writeln!(
            description,
            "```\n \
        Acc: #Scores |  Ratio | % misses\n\
        --------------+--------+---------"
        );
        let mut all_scores = Vec::with_capacity(6);
        let mut all_ratios = Vec::with_capacity(6);
        let mut all_misses = Vec::with_capacity(6);
        for (acc, c) in categories.into_iter() {
            if c.scores > 0 {
                let scores = c.scores;
                let ratio = c.ratio();
                let misses = c.miss_percent();
                let _ = writeln!(
                    description,
                    "{}{:>2}%: {:>7} | {:>6} | {:>7}%",
                    if acc < 100 { ">" } else { "" },
                    acc,
                    scores,
                    ratio,
                    misses,
                );
                all_scores.push(scores as i16);
                all_ratios.push(ratio);
                all_misses.push(misses);
            }
        }
        let previous_ratios = {
            let data = data.read().await;
            let mysql = data.get::<MySQL>().expect("Could not get MySQL");
            mysql.update_ratios(
                &user.username,
                all_scores.iter().join(","),
                all_ratios.iter().join(","),
                all_misses.iter().join(","),
            )
        };
        if let Some(ratios) = previous_ratios {
            if ratios.scores != all_scores
                || ratios.ratios != all_ratios
                || ratios.misses != all_misses
            {
                let _ = writeln!(description, "--------------+--------+---------");
                accs.push(100);
                for (i, acc) in accs.iter().enumerate() {
                    let any_changes = match (ratios.scores.get(i), all_scores.get(i)) {
                        (Some(new), Some(old)) => new != old,
                        (None, Some(_)) => true,
                        (Some(_), None) => true,
                        (None, None) => false,
                    } || match (ratios.ratios.get(i), all_ratios.get(i)) {
                        (Some(new), Some(old)) => (new - old).abs() >= 0.0005,
                        (None, Some(_)) => true,
                        (Some(_), None) => true,
                        (None, None) => false,
                    } || match (ratios.misses.get(i), all_misses.get(i)) {
                        (Some(new), Some(old)) => (new - old).abs() >= 0.0005,
                        (None, Some(_)) => true,
                        (Some(_), None) => true,
                        (None, None) => false,
                    };
                    if any_changes {
                        let _ = writeln!(
                            description,
                            "{}{:>2}%: {:>+7} | {:>+6} | {:>+7}%",
                            if *acc < 100 { ">" } else { "" },
                            acc,
                            *all_scores.get(i).unwrap_or_else(|| &0)
                                - *ratios.scores.get(i).unwrap_or_else(|| &0),
                            round_precision(
                                *all_ratios.get(i).unwrap_or_else(|| &0.0)
                                    - *ratios.ratios.get(i).unwrap_or_else(|| &0.0),
                                3
                            ),
                            round_precision(
                                *all_misses.get(i).unwrap_or_else(|| &0.0)
                                    - *ratios.misses.get(i).unwrap_or_else(|| &0.0),
                                3
                            ),
                        );
                    }
                }
            }
        }
        description.push_str("```");
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.description = Some(description);
        Ok(result)
    }

    //
    // roleassign
    //
    pub async fn create_roleassign(
        msg: Message,
        guild: GuildId,
        role: RoleId,
        cache: &CacheRwLock,
    ) -> Self {
        let mut result = Self::default();
        let description = format!(
            "Whoever reacts to {author}'s [message]\
            (https://discordapp.com/channels/{guild}/{channel}/{msg})\n\
            ```\n{content}\n```\n\
            in {channel_mention} will be assigned the {role_mention} role!",
            author = msg.author.mention(),
            guild = guild,
            channel = msg.channel_id,
            msg = msg.id,
            content = content_safe(cache, &msg.content, &ContentSafeOptions::default()).await,
            channel_mention = msg.channel_id.mention(),
            role_mention = role.mention(),
        );
        result.description = Some(description);
        result
    }

    //
    // scores
    //
    pub async fn create_scores<D>(
        user: User,
        map: Beatmap,
        scores: Vec<Score>,
        cache_data: D,
    ) -> Result<Self, Error>
    where
        D: CacheData,
    {
        let mut result = Self::default();
        let title = map.to_string();
        let title_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id);
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        if scores.is_empty() {
            result.description = Some("No scores found".to_string());
        }
        let mut fields = Vec::new();
        for (i, score) in scores.into_iter().enumerate() {
            let (stars, pp) = {
                let data = Arc::clone(cache_data.data());
                let pp_provider = match PPProvider::new(&score, &map, Some(data)).await {
                    Ok(provider) => provider,
                    Err(why) => {
                        return Err(Error::Custom(format!(
                            "Something went wrong while creating PPProvider: {}",
                            why
                        )))
                    }
                };
                (
                    util::get_stars(pp_provider.stars()),
                    util::get_pp(&score, &pp_provider, map.mode),
                )
            };
            let cache = cache_data.cache().clone();
            let mut name = format!(
                "**{idx}.** {grade}\t[{stars}]\t{score}\t({acc})",
                idx = i + 1,
                grade = util::get_grade_completion_mods(&score, &map, cache).await,
                stars = stars,
                score = with_comma_u64(score.score as u64),
                acc = util::get_acc(&score, map.mode),
            );
            if map.mode == GameMode::MNA {
                let _ = write!(name, "\t{}", util::get_keys(&score.enabled_mods, &map));
            }
            let value = format!(
                "{pp}\t[ {combo} ]\t {hits}\t{ago}",
                pp = pp,
                combo = util::get_combo(&score, &map),
                hits = util::get_hits(&score, map.mode),
                ago = how_long_ago(&score.date)
            );
            fields.push((name, value, false));
        }
        result.title_text = Some(title);
        result.title_url = Some(title_url);
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.footer_icon = Some(footer_url);
        result.footer_text = Some(footer_text);
        result.thumbnail = Some(thumbnail);
        result.fields = Some(fields);
        Ok(result)
    }

    //
    // twitch notification
    //
    pub fn create_twitch_stream_notif(stream: &TwitchStream, user: &TwitchUser) -> Self {
        let mut result = Self::default();
        result.author_text = Some(String::from("Now live on twitch:"));
        result.title_text = Some(stream.username.clone());
        result.title_url = Some(format!("{}{}", TWITCH_BASE, stream.username));
        result.image_url = Some(stream.thumbnail_url.clone());
        result.thumbnail = Some(user.image_url.clone());
        result.description = Some(stream.title.clone());
        result
    }

    //
    // top
    //
    pub async fn create_top<'i, S, D>(
        user: &User,
        scores_data: S,
        mode: GameMode,
        pages: (usize, usize),
        cache_data: D,
    ) -> Result<Self, Error>
    where
        S: Iterator<Item = &'i (usize, Score, Beatmap)>,
        D: CacheData,
    {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(user);
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(512);
        for (idx, score, map) in scores_data {
            let cache = cache_data.cache().clone();
            let grade = { osu::grade_emote(score.grade, cache).await };
            let (stars, pp) = {
                let data = Arc::clone(cache_data.data());
                let pp_provider = match PPProvider::new(&score, &map, Some(data)).await {
                    Ok(provider) => provider,
                    Err(why) => {
                        return Err(Error::Custom(format!(
                            "Something went wrong while creating PPProvider: {}",
                            why
                        )))
                    }
                };
                (
                    util::get_stars(pp_provider.stars()),
                    util::get_pp(score, &pp_provider, mode),
                )
            };
            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                idx = idx,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
                mods = util::get_mods(&score.enabled_mods),
                stars = stars,
                grade = grade,
                pp = pp,
                acc = util::get_acc(&score, mode),
                score = with_comma_u64(score.score as u64),
                combo = util::get_combo(&score, &map),
                hits = util::get_hits(&score, mode),
                ago = how_long_ago(&score.date)
            );
        }
        description.pop();
        let footer_text = format!("Page {}/{}", pages.0, pages.1);
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.description = Some(description);
        result.footer_text = Some(footer_text);
        Ok(result)
    }

    //
    //  whatif
    //
    pub fn create_whatif(user: User, scores: Vec<Score>, _mode: GameMode, pp: f32) -> Self {
        let mut result = Self::default();
        let (author_icon, author_url, author_text) = get_user_author(&user);
        let title = format!(
            "What if {name} got a new {pp_given}pp score?",
            name = user.username,
            pp_given = pp
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let pp_values: Vec<f32> = scores
            .iter()
            .map(|score| *score.pp.as_ref().unwrap())
            .collect();
        let description = if pp < pp_values[pp_values.len() - 1] {
            format!(
                "A {pp_given}pp play wouldn't even be in {name}'s top 100 plays.\n\
                 There would not be any significant pp change.",
                pp_given = pp,
                name = user.username
            )
        } else {
            let mut actual: f32 = 0.0;
            let mut factor: f32 = 1.0;
            for score in pp_values.iter() {
                actual += score * factor;
                factor *= 0.95;
            }
            let bonus = user.pp_raw - actual;
            let mut potential = 0.0;
            let mut used = false;
            let mut new_pos = None;
            let mut factor = 1.0;
            for (i, pp_value) in pp_values.iter().enumerate().take(pp_values.len() - 1) {
                if !used && *pp_value < pp {
                    used = true;
                    potential += pp * factor;
                    factor *= 0.95;
                    new_pos = Some(i + 1);
                }
                potential += pp_value * factor;
                factor *= 0.95;
            }
            format!(
                "A {pp}pp play would be {name}'s #{num} best play.\n\
                 Their pp would change by **+{pp_change}** to **{new_pp}pp**.",
                pp = round(pp),
                name = user.username,
                num = new_pos.unwrap(),
                pp_change = round(potential + bonus - user.pp_raw),
                new_pp = round(potential + bonus)
            )
        };
        result.author_icon = Some(author_icon);
        result.author_url = Some(author_url);
        result.author_text = Some(author_text);
        result.thumbnail = Some(thumbnail);
        result.title_text = Some(title);
        result.description = Some(description);
        result
    }
}

// -------------------
// Auxiliary functions
// -------------------

fn get_user_author(user: &User) -> (String, String, String) {
    let icon = format!("{}/images/flags/{}.png", HOMEPAGE, user.country);
    let url = format!("{}u/{}", HOMEPAGE, user.user_id);
    let text = format!(
        "{name}: {pp}pp (#{global} {country}{national})",
        name = user.username,
        pp = round_and_comma(user.pp_raw),
        global = with_comma_u64(user.pp_rank as u64),
        country = user.country,
        national = user.pp_country_rank
    );
    (icon, url, text)
}

fn add_team(description: &mut String, team: Vec<(String, f32)>, with_mvp: bool) {
    for (i, (name, cost)) in team.into_iter().enumerate() {
        let _ = writeln!(
            description,
            "**{idx}**: [{name}]({base}users/{name_r}) - **{cost}**{crown}",
            idx = i + 1,
            name = name,
            base = HOMEPAGE,
            name_r = name.replace(" ", "%20"),
            cost = round(cost),
            crown = if i == 0 && with_mvp { " :crown:" } else { "" },
        );
    }
}

pub async fn get_pp(
    mod_map: &mut HashMap<u32, f32>,
    score: &ScraperScore,
    map: &Beatmap,
    data: Arc<RwLock<TypeMap>>,
) -> Result<String, Error> {
    let bits = score.enabled_mods.as_bits();
    let actual = if score.pp.is_some() {
        score.pp
    } else {
        match map.mode {
            GameMode::CTB => None,
            GameMode::STD | GameMode::TKO => {
                Some(PPProvider::calculate_oppai_pp(score, map).await?)
            }
            GameMode::MNA => {
                Some(PPProvider::calculate_mania_pp(score, map, Arc::clone(&data)).await?)
            }
        }
    };
    #[allow(clippy::map_entry)]
    let max = if mod_map.contains_key(&bits) {
        mod_map.get(&bits).copied()
    } else if map.mode == GameMode::CTB {
        None
    } else {
        let max = PPProvider::calculate_max(&map, &score.enabled_mods, Some(data)).await?;
        mod_map.insert(bits, max);
        Some(max)
    };
    Ok(format!(
        "**{}**/{}PP",
        actual.map_or_else(|| "-".to_string(), |pp| round(pp).to_string()),
        max.map_or_else(|| "-".to_string(), |pp| round(pp).to_string())
    ))
}

pub fn get_combo(score: &ScraperScore, map: &Beatmap) -> String {
    let mut combo = format!("**{}x**/", score.max_combo.to_string());
    let _ = if let Some(amount) = map.max_combo {
        write!(combo, "{}x", amount)
    } else {
        write!(
            combo,
            " {} miss{}",
            score.count_miss,
            if score.count_miss != 1 { "es" } else { "" }
        )
    };
    combo
}

// -----------------
// Auxiliary structs
// -----------------

#[derive(Default)]
struct RatioCategory {
    pub scores: u8,
    pub count_geki: u32,
    pub count_300: u32,
    pub count_miss: u32,
    pub count_objects: u32,
}

impl RatioCategory {
    fn add_score(&mut self, s: &Score) {
        self.scores += 1;
        self.count_geki += s.count_geki;
        self.count_300 += s.count300;
        self.count_miss += s.count_miss;
        self.count_objects +=
            s.count_geki + s.count300 + s.count_katu + s.count100 + s.count50 + s.count_miss;
    }

    fn ratio(&self) -> f32 {
        if self.count_300 == 0 {
            self.count_geki as f32
        } else {
            round_precision(self.count_geki as f32 / self.count_300 as f32, 3)
        }
    }

    fn miss_percent(&self) -> f32 {
        if self.count_objects > 0 {
            round_precision(
                100.0 * self.count_miss as f32 / self.count_objects as f32,
                3,
            )
        } else {
            0.0
        }
    }
}

/// Providing a hashable, comparable alternative to f32 to put as key in a BTreeMap
#[derive(Hash, Eq, PartialEq)]
struct F32T {
    integral: u32,
    fractional: u32,
}

impl F32T {
    fn new(val: f32) -> Self {
        Self {
            integral: val.trunc() as u32,
            fractional: (val.fract() * 10_000.0) as u32,
        }
    }
}

impl F32T {
    fn to_f32(&self) -> f32 {
        self.integral as f32 + self.fractional as f32 / 10_000.0
    }
}

impl Ord for F32T {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.integral.cmp(&other.integral) {
            Ordering::Equal => self.fractional.cmp(&other.fractional),
            order => order,
        }
    }
}

impl PartialOrd for F32T {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct ProfileResult {
    min_acc: f32,
    max_acc: f32,
    avg_acc: f32,

    min_pp: f32,
    max_pp: f32,
    avg_pp: f32,

    min_combo: u32,
    max_combo: u32,
    avg_combo: u32,
    map_combo: u32,

    min_len: u32,
    max_len: u32,
    avg_len: u32,

    mappers: Vec<(String, u32, f32)>,

    mod_combs_count: Option<Vec<(GameMods, u32)>>,
    mod_combs_pp: Option<Vec<(GameMods, f32)>>,
    mods_count: Vec<(GameMod, u32)>,
    mods_pp: Vec<(GameMod, f32)>,
}

impl ProfileResult {
    fn calc(mode: GameMode, tuples: Vec<(Score, Beatmap)>) -> Self {
        let (mut min_acc, mut max_acc, mut avg_acc) = (f32::MAX, 0.0_f32, 0.0);
        let (mut min_pp, mut max_pp, mut avg_pp) = (f32::MAX, 0.0_f32, 0.0);
        let (mut min_combo, mut max_combo, mut avg_combo, mut map_combo) = (u32::MAX, 0, 0, 0);
        let (mut min_len, mut max_len, mut avg_len) = (u32::MAX, 0, 0);
        let len = tuples.len() as f32;
        let mut mappers = HashMap::with_capacity(len as usize);
        let mut mod_combs = HashMap::with_capacity(5);
        let mut mods = HashMap::with_capacity(5);
        let mut factor = 1.0;
        let mut mult_mods = false;
        for (score, map) in tuples {
            let acc = score.accuracy(mode);
            min_acc = min_acc.min(acc);
            max_acc = max_acc.max(acc);
            avg_acc += acc;

            if let Some(pp) = score.pp {
                min_pp = min_pp.min(pp);
                max_pp = max_pp.max(pp);
                avg_pp += pp;
            }

            min_combo = min_combo.min(score.max_combo);
            max_combo = max_combo.max(score.max_combo);
            avg_combo += score.max_combo;

            if let Some(combo) = map.max_combo {
                map_combo += combo;
            }

            min_len = min_len.min(map.seconds_drain);
            max_len = max_len.max(map.seconds_drain);
            avg_len += map.seconds_drain;

            let mut mapper = mappers.entry(map.creator).or_insert((0, 0.0));
            let weighted_pp = score.pp.unwrap_or(0.0) * factor;
            factor *= 0.95;
            mapper.0 += 1;
            mapper.1 += weighted_pp;
            {
                let mut mod_comb = mod_combs
                    .entry(score.enabled_mods.clone())
                    .or_insert((0, 0.0));
                mod_comb.0 += 1;
                mod_comb.1 += weighted_pp;
            }
            if score.enabled_mods.is_empty() {
                let mut nm = mods.entry(GameMod::NoMod).or_insert((0, 0.0));
                nm.0 += 1;
                nm.1 += weighted_pp;
            } else {
                mult_mods |= score.enabled_mods.len() > 1;
                for m in score.enabled_mods {
                    let mut r#mod = mods.entry(m).or_insert((0, 0.0));
                    r#mod.0 += 1;
                    r#mod.1 += weighted_pp;
                }
            }
        }
        avg_acc /= len;
        avg_pp /= len;
        avg_combo /= len as u32;
        avg_len /= len as u32;
        map_combo /= len as u32;
        mod_combs
            .values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);
        mods.values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);
        let mut mappers: Vec<_> = mappers
            .into_iter()
            .map(|(name, (count, pp))| (name, count, pp))
            .collect();
        mappers.sort_by(
            |(_, count_a, pp_a), (_, count_b, pp_b)| match count_b.cmp(&count_a) {
                Equal => pp_b.partial_cmp(pp_a).unwrap_or(Equal),
                other => other,
            },
        );
        mappers = mappers[..5.min(mappers.len())].to_vec();
        let (mod_combs_count, mod_combs_pp) = if mult_mods {
            let mut mod_combs_count: Vec<_> = mod_combs
                .iter()
                .map(|(name, (count, _))| (name.clone(), *count))
                .collect();
            mod_combs_count.sort_by(|a, b| b.1.cmp(&a.1));
            let mut mod_combs_pp: Vec<_> = mod_combs
                .into_iter()
                .map(|(name, (_, avg))| (name, avg))
                .collect();
            mod_combs_pp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));
            (Some(mod_combs_count), Some(mod_combs_pp))
        } else {
            (None, None)
        };
        let mut mods_count: Vec<_> = mods
            .iter()
            .map(|(name, (count, _))| (*name, *count))
            .collect();
        mods_count.sort_by(|a, b| b.1.cmp(&a.1));
        let mut mods_pp: Vec<_> = mods
            .into_iter()
            .map(|(name, (_, avg))| (name, avg))
            .collect();
        mods_pp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));
        Self {
            min_acc,
            max_acc,
            avg_acc,
            min_pp,
            max_pp,
            avg_pp,
            min_combo,
            max_combo,
            avg_combo,
            map_combo,
            min_len,
            max_len,
            avg_len,
            mappers,
            mod_combs_count,
            mod_combs_pp,
            mods_count,
            mods_pp,
        }
    }
}
