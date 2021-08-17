use super::{MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32};
use crate::{
    embeds::{EmbedData, ProfileEmbed},
    pagination::ProfilePagination,
    tracking::process_tracking,
    util::{constants::OSU_API_ISSUE, osu::BonusPP, ApplicationCommandExt, CowUtils, MessageExt},
    Args, BotResult, CommandData, Context, Error, MessageBuilder, Name,
};

use chrono::Datelike;
use futures::{
    future::TryFutureExt,
    stream::{FuturesUnordered, TryStreamExt},
};
use hashbrown::HashMap;
use image::{imageops::FilterType::Lanczos3, load_from_memory, png::PngEncoder, ColorType};
use plotters::prelude::*;
use reqwest::Response;
use rosu_v2::prelude::{GameMode, GameMods, MonthlyCount, OsuError, Score, User, UserStatistics};
use std::{
    borrow::Cow,
    cmp::{Ordering::Equal, PartialOrd},
    collections::{BTreeMap, HashMap as StdHashMap},
    mem,
    sync::Arc,
};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

async fn _profile(ctx: Arc<Context>, data: CommandData<'_>, args: ProfileArgs) -> BotResult<()> {
    let ProfileArgs { name, mode, kind } = args;

    let author_id = data.author()?.id;

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(author_id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    // Retrieve the user and their top scores
    let user_fut = super::request_user(&ctx, &name, Some(mode));
    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .best()
        .mode(mode)
        .limit(100);

    let (user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    process_tracking(&ctx, mode, &mut scores, Some(&user)).await;

    // Store maps in DB
    if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
        unwind_error!(warn, why, "Error while storing profile maps in DB: {}");
    }

    let mut profile_data = ProfileData::new(user, scores);

    // Draw the graph
    let graph = match graphs(&mut profile_data.user).await {
        Ok(graph_option) => graph_option,
        Err(why) => {
            unwind_error!(warn, why, "Error while creating profile graph: {}");

            None
        }
    };

    // Create the embed
    let embed_data = ProfileEmbed::get_or_create(&ctx, kind, &mut profile_data).await;

    // Send the embed
    let embed = embed_data.as_builder().build();
    let mut builder = MessageBuilder::new().embed(embed);

    if let Some(bytes) = graph.as_deref() {
        builder = builder.file("profile_graph.png", bytes);
    }

    let response_raw = data.create_message(&ctx, builder).await?;
    let response = data.get_response(&ctx, response_raw).await?;

    // Pagination
    let pagination = ProfilePagination::new(response, profile_data, kind);
    let owner = author_id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (profile): {}")
        }
    });

    Ok(())
}

impl ProfileEmbed {
    pub async fn get_or_create<'map>(
        ctx: &Context,
        kind: ProfileSize,
        profile_data: &'map mut ProfileData,
    ) -> &'map Self {
        if profile_data.embeds.get(kind).is_none() {
            let user = &profile_data.user;

            let data = match kind {
                ProfileSize::Compact => {
                    let max_pp = profile_data
                        .scores
                        .first()
                        .and_then(|score| score.pp)
                        .unwrap_or(0.0);

                    ProfileEmbed::compact(user, max_pp)
                }
                ProfileSize::Medium => {
                    let scores = &profile_data.scores;

                    if profile_data.profile_result.is_none() && !scores.is_empty() {
                        let stats = user.statistics.as_ref().unwrap();

                        profile_data.profile_result =
                            Some(ProfileResult::calc(user.mode, scores, stats));
                    }

                    let bonus_pp = profile_data
                        .profile_result
                        .as_ref()
                        .map_or(0.0, |result| result.bonus_pp);

                    ProfileEmbed::medium(user, bonus_pp)
                }
                ProfileSize::Full => {
                    let scores = &profile_data.scores;
                    let mode = user.mode;
                    let own_top_scores = profile_data.own_top_scores();

                    let globals_count = match profile_data.globals_count.as_ref() {
                        Some(counts) => counts,
                        None => match super::get_globals_count(ctx, &user.username, mode).await {
                            Ok(globals_count) => profile_data.globals_count.insert(globals_count),
                            Err(why) => {
                                unwind_error!(
                                    error,
                                    why,
                                    "Error while requesting globals count: {}"
                                );

                                profile_data.globals_count.insert(BTreeMap::new())
                            }
                        },
                    };

                    if profile_data.profile_result.is_none() && !scores.is_empty() {
                        let stats = user.statistics.as_ref().unwrap();

                        profile_data.profile_result =
                            Some(ProfileResult::calc(mode, scores, stats));
                    }

                    let profile_result = profile_data.profile_result.as_ref();

                    ProfileEmbed::full(&user, profile_result, globals_count, own_top_scores)
                }
            };

            profile_data.embeds.insert(kind, data);
        }

        // Annoying NLL workaround
        //   - https://github.com/rust-lang/rust/issues/43234
        //   - https://github.com/rust-lang/rust/issues/51826
        profile_data.embeds.get(kind).unwrap()
    }
}

#[command]
#[short_desc("Display statistics of a user")]
#[long_desc(
    "Display statistics of a user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`. Defaults to `compact`."
)]
#[usage("[username] [size=compact/medium/full]")]
#[example("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profile")]
async fn osu(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, GameMode::STD) {
                Ok(profile_args) => {
                    _profile(ctx, CommandData::Message { msg, args, num }, profile_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_profile(ctx, command).await,
    }
}

#[command]
#[short_desc("Display statistics of a mania user")]
#[long_desc(
    "Display statistics of a mania user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`. Defaults to `compact`."
)]
#[usage("[username] [size=compact/medium/full]")]
#[example("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profilemania", "maniaprofile", "profilem")]
async fn mania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, GameMode::MNA) {
                Ok(profile_args) => {
                    _profile(ctx, CommandData::Message { msg, args, num }, profile_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_profile(ctx, command).await,
    }
}

#[command]
#[short_desc("Display statistics of a taiko user")]
#[long_desc(
    "Display statistics of a taiko user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`. Defaults to `compact`."
)]
#[usage("[username] [size=compact/medium/full]")]
#[example("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profiletaiko", "taikoprofile", "profilet")]
async fn taiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, GameMode::TKO) {
                Ok(profile_args) => {
                    _profile(ctx, CommandData::Message { msg, args, num }, profile_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_profile(ctx, command).await,
    }
}

#[command]
#[short_desc("Display statistics of a ctb user")]
#[long_desc(
    "Display statistics of a ctb user.\n\
    You can choose between `compact`, `medium`, and `full` embed \
    by specifying the argument `size=...`. Defaults to `compact`."
)]
#[usage("[username] [size=compact/medium/full]")]
#[example("badewanne3", "peppy size=full", "size=compact \"freddie benson\"")]
#[aliases("profilectb", "ctbprofile", "profilec")]
async fn ctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match ProfileArgs::args(&ctx, &mut args, GameMode::CTB) {
                Ok(profile_args) => {
                    _profile(ctx, CommandData::Message { msg, args, num }, profile_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_profile(ctx, command).await,
    }
}

#[derive(Copy, Clone)]
pub enum ProfileSize {
    Compact,
    Medium,
    Full,
}

impl ProfileSize {
    pub fn minimize(&self) -> Option<Self> {
        match self {
            ProfileSize::Compact => None,
            ProfileSize::Medium => Some(ProfileSize::Compact),
            ProfileSize::Full => Some(ProfileSize::Medium),
        }
    }

    pub fn expand(&self) -> Option<Self> {
        match self {
            ProfileSize::Compact => Some(ProfileSize::Medium),
            ProfileSize::Medium => Some(ProfileSize::Full),
            ProfileSize::Full => None,
        }
    }
}

impl Default for ProfileSize {
    fn default() -> Self {
        Self::Compact
    }
}

struct ProfileArgs {
    name: Option<Name>,
    mode: GameMode,
    kind: ProfileSize,
}

impl ProfileArgs {
    fn args(ctx: &Context, args: &mut Args, mode: GameMode) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut kind = None;

        for arg in args.take(2).map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = &arg[idx + 1..];

                match key {
                    "size" => {
                        kind = match value {
                            "compact" | "small" => Some(ProfileSize::Compact),
                            "medium" => Some(ProfileSize::Medium),
                            "full" | "big" => Some(ProfileSize::Full),
                            _ => {
                                let content = "Could not parse size. Must be either `compact`, `medium`, or `full`.";

                                return Err(content.into());
                            }
                        };
                    }
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `size`.",
                            key
                        );

                        return Err(content.into());
                    }
                }
            } else {
                name = Some(Args::try_link_name(ctx, arg.as_ref())?);
            }
        }

        let args = Self {
            name,
            mode,
            kind: kind.unwrap_or_default(),
        };

        Ok(args)
    }

    fn slash(ctx: &Context, command: &mut ApplicationCommand) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut mode = None;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => mode = parse_mode_option!(value, "profile"),
                    "size" => match value.as_str() {
                        "compact" => kind = Some(ProfileSize::Compact),
                        "medium" => kind = Some(ProfileSize::Medium),
                        "full" => kind = Some(ProfileSize::Full),
                        _ => bail_cmd_option!("profile size", string, value),
                    },
                    "name" => username = Some(value.into()),
                    "discord" => username = parse_discord_option!(ctx, value, "profile"),
                    _ => bail_cmd_option!("profile", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("profile", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("profile", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("profile", subcommand, name)
                }
            }
        }

        let args = Self {
            name: username,
            mode: mode.unwrap_or(GameMode::STD),
            kind: kind.unwrap_or_default(),
        };

        Ok(Ok(args))
    }
}

pub async fn slash_profile(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match ProfileArgs::slash(&ctx, &mut command)? {
        Ok(args) => _profile(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_profile_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "profile".to_owned(),
        default_permission: None,
        description: "Display statistics of a user".to_owned(),
        id: None,
        options: vec![
            CommandOption::String(ChoiceCommandOptionData {
                choices: super::mode_choices(),
                description: "Specify a gamemode".to_owned(),
                name: "mode".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![
                    CommandOptionChoice::String {
                        name: "compact".to_owned(),
                        value: "compact".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "medium".to_owned(),
                        value: "medium".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "full".to_owned(),
                        value: "full".to_owned(),
                    },
                ],
                description: "Choose an embed size".to_owned(),
                name: "size".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify a username".to_owned(),
                name: "name".to_owned(),
                required: false,
            }),
            CommandOption::User(BaseCommandOptionData {
                description: "Specify a linked discord user".to_owned(),
                name: "discord".to_owned(),
                required: false,
            }),
        ],
    }
}

pub struct ProfileData {
    user: User,
    scores: Vec<Score>,
    embeds: ProfileEmbedMap,
    profile_result: Option<ProfileResult>,
    globals_count: Option<BTreeMap<usize, Cow<'static, str>>>,
}

impl ProfileData {
    fn new(user: User, scores: Vec<Score>) -> Self {
        Self {
            user,
            scores,
            embeds: ProfileEmbedMap::default(),
            profile_result: None,
            globals_count: None,
        }
    }

    /// Check if user has top scores on their own maps
    pub fn own_top_scores(&self) -> usize {
        let ranked_maps_count =
            self.user.ranked_mapset_count.unwrap() + self.user.loved_mapset_count.unwrap();

        if ranked_maps_count > 0 {
            self.scores
                .iter()
                .filter(|score| score.mapset.as_ref().unwrap().creator_name == self.user.username)
                .count()
        } else {
            0
        }
    }
}

#[derive(Default)]
pub struct ProfileEmbedMap {
    compact: Option<ProfileEmbed>,
    medium: Option<ProfileEmbed>,
    full: Option<ProfileEmbed>,
}

impl ProfileEmbedMap {
    pub fn get(&self, kind: ProfileSize) -> Option<&ProfileEmbed> {
        match kind {
            ProfileSize::Compact => self.compact.as_ref(),
            ProfileSize::Medium => self.medium.as_ref(),
            ProfileSize::Full => self.full.as_ref(),
        }
    }

    pub fn insert(&mut self, kind: ProfileSize, embed: ProfileEmbed) -> &ProfileEmbed {
        match kind {
            ProfileSize::Compact => self.compact.insert(embed),
            ProfileSize::Medium => self.medium.insert(embed),
            ProfileSize::Full => self.full.insert(embed),
        }
    }
}

pub struct ProfileResult {
    pub mode: GameMode,

    pub acc: MinMaxAvgF32,
    pub pp: MinMaxAvgF32,
    pub bonus_pp: f32,
    pub map_combo: u32,
    pub combo: MinMaxAvgU32,
    pub map_len: MinMaxAvgU32,

    pub mappers: Vec<(String, u32, f32)>,
    pub mod_combs_count: Option<Vec<(GameMods, u32)>>,
    pub mod_combs_pp: Vec<(GameMods, f32)>,
    pub mods_count: Vec<(GameMods, u32)>,
}

impl ProfileResult {
    fn calc(mode: GameMode, scores: &[Score], stats: &UserStatistics) -> Self {
        let mut acc = MinMaxAvgF32::new();
        let mut pp = MinMaxAvgF32::new();
        let mut combo = MinMaxAvgU32::new();
        let mut map_len = MinMaxAvgF32::new();
        let mut map_combo = 0;
        let mut mappers = StdHashMap::with_capacity(scores.len());
        let len = scores.len() as f32;
        let mut mod_combs = HashMap::with_capacity(5);
        let mut mods = HashMap::with_capacity(5);
        let mut mult_mods = false;
        let mut bonus_pp = BonusPP::new();

        for (i, score) in scores.iter().enumerate() {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            acc.add(score.accuracy);

            if let Some(score_pp) = score.pp {
                pp.add(score_pp);
            }

            if let Some(weighted_pp) = score.weight.map(|w| w.pp) {
                bonus_pp.update(weighted_pp, i);

                let mut mapper = mappers.entry(&mapset.creator_name).or_insert((0, 0.0));
                mapper.0 += 1;
                mapper.1 += weighted_pp;

                let mut mod_comb = mod_combs.entry(score.mods).or_insert((0, 0.0));
                mod_comb.0 += 1;
                mod_comb.1 += weighted_pp;
            }

            combo.add(score.max_combo);

            if let Some(combo) = map.max_combo {
                map_combo += combo;
            }

            let seconds_drain = if score.mods.contains(GameMods::DoubleTime) {
                map.seconds_drain as f32 / 1.5
            } else if score.mods.contains(GameMods::HalfTime) {
                map.seconds_drain as f32 * 1.5
            } else {
                map.seconds_drain as f32
            };

            map_len.add(seconds_drain);

            if score.mods.is_empty() {
                *mods.entry(GameMods::NoMod).or_insert(0) += 1;
            } else {
                mult_mods |= score.mods.len() > 1;

                for m in score.mods {
                    *mods.entry(m).or_insert(0) += 1;
                }
            }
        }

        map_combo /= len as u32;

        mod_combs
            .values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);

        mods.values_mut()
            .for_each(|count| *count = (*count as f32 * 100.0 / len) as u32);

        let mut mappers: Vec<_> = mappers
            .into_iter()
            .map(|(name, (count, pp))| (name.to_owned(), count as u32, pp))
            .collect();

        mappers.sort_unstable_by(|(_, count_a, pp_a), (_, count_b, pp_b)| {
            match count_b.cmp(count_a) {
                Equal => pp_b.partial_cmp(pp_a).unwrap_or(Equal),
                other => other,
            }
        });

        mappers = mappers[..5.min(mappers.len())].to_vec();

        let mod_combs_count = if mult_mods {
            let mut mod_combs_count: Vec<_> = mod_combs
                .iter()
                .map(|(name, (count, _))| (*name, *count))
                .collect();

            mod_combs_count.sort_unstable_by(|a, b| b.1.cmp(&a.1));

            Some(mod_combs_count)
        } else {
            None
        };

        let mod_combs_pp = {
            let mut mod_combs_pp: Vec<_> = mod_combs
                .into_iter()
                .map(|(name, (_, avg))| (name, avg))
                .collect();

            mod_combs_pp.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Equal));

            mod_combs_pp
        };

        let mut mods_count: Vec<_> = mods.into_iter().collect();
        mods_count.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        Self {
            mode,
            acc,
            pp,
            bonus_pp: bonus_pp.calculate(stats),
            combo,
            map_combo,
            map_len: map_len.into(),
            mappers,
            mod_combs_count,
            mod_combs_pp,
            mods_count,
        }
    }
}

const W: u32 = 1350;
const H: u32 = 350;

async fn graphs(user: &mut User) -> Result<Option<Vec<u8>>, Error> {
    let mut monthly_playcount = mem::replace(&mut user.monthly_playcounts, None).unwrap();
    let badges = mem::replace(&mut user.badges, None).unwrap();

    if monthly_playcount.len() < 2 {
        return Ok(None);
    }

    static LEN: usize = W as usize * H as usize;
    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3

    {
        // Request all badge images
        let badges = match badges.is_empty() {
            true => Vec::new(),
            false => {
                badges
                    .iter()
                    .map(|badge| {
                        reqwest::get(&badge.image_url)
                            .and_then(Response::bytes)
                            .map_ok(|bytes| bytes.to_vec())
                    })
                    .collect::<FuturesUnordered<_>>()
                    .try_collect()
                    .await?
            }
        };

        // Setup total canvas
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;

        // Draw badges if there are any
        let canvas = if badges.is_empty() {
            root
        } else {
            let max_badges_per_row = 10;
            let margin = 5;
            let inner_margin = 3;
            let badge_count = badges.len() as u32;
            let badge_rows = ((badge_count - 1) / max_badges_per_row) + 1;
            let badge_total_height = (badge_rows * 60).min(H / 2);
            let badge_height = badge_total_height / badge_rows;
            let (top, bottom) = root.split_vertically(badge_total_height);
            let mut rows = Vec::with_capacity(badge_rows as usize);
            let mut last = top;

            for _ in 0..badge_rows {
                let (curr, remain) = last.split_vertically(badge_height);
                rows.push(curr);
                last = remain;
            }

            let badge_width =
                (W - 2 * margin - (max_badges_per_row - 1) * inner_margin) / max_badges_per_row;

            // Draw each row of badges
            for (row, chunk) in badges.chunks(max_badges_per_row as usize).enumerate() {
                let x_offset = (max_badges_per_row - chunk.len() as u32) * badge_width / 2;

                let mut chart_row = ChartBuilder::on(&rows[row])
                    .margin(margin)
                    .build_cartesian_2d(0..W, 0..badge_height)?;

                chart_row
                    .configure_mesh()
                    .disable_x_axis()
                    .disable_y_axis()
                    .disable_x_mesh()
                    .disable_y_mesh()
                    .draw()?;

                for (idx, badge) in chunk.iter().enumerate() {
                    let badge_img =
                        load_from_memory(badge)?.resize_exact(badge_width, badge_height, Lanczos3);

                    let x = x_offset + idx as u32 * badge_width + idx as u32 * inner_margin;
                    let y = badge_height;
                    let elem: BitMapElement<_> = ((x, y), badge_img).into();
                    chart_row.draw_series(std::iter::once(elem))?;
                }
            }

            bottom
        };

        let replays = user.replays_watched_counts.as_mut().unwrap();

        // Spoof missing months
        // Making use of the fact that the dates are always of the form YYYY-MM-01
        let first_date = monthly_playcount.first().unwrap().start_date;
        let mut curr_month = first_date.month();
        let mut curr_year = first_date.year();

        let dates = monthly_playcount
            .iter()
            .map(|date_count| date_count.start_date)
            .enumerate()
            .collect::<Vec<_>>()
            .into_iter();

        let mut inserted = 0;

        for (i, date) in dates {
            while date.month() != curr_month || date.year() != curr_year {
                let spoofed_date = date
                    .with_month(curr_month)
                    .unwrap()
                    .with_year(curr_year)
                    .unwrap();

                let count = MonthlyCount {
                    start_date: spoofed_date,
                    count: 0,
                };

                monthly_playcount.insert(inserted + i, count);
                inserted += 1;
                curr_month += 1;

                if curr_month == 13 {
                    curr_month = 1;
                    curr_year += 1;
                }
            }

            curr_month += 1;

            if curr_month == 13 {
                curr_month = 1;
                curr_year += 1;
            }
        }

        // Spoof missing replays
        let dates = monthly_playcount
            .iter()
            .map(|date_count| date_count.start_date)
            .enumerate();

        for (i, date) in dates {
            let cond = replays
                .get(i)
                .map(|date_count| date_count.start_date == date);

            let count = MonthlyCount {
                start_date: date,
                count: 0,
            };

            if let None | Some(false) = cond {
                replays.insert(i, count);
            }
        }

        let left_first = monthly_playcount.first().unwrap().start_date;
        let left_last = monthly_playcount.last().unwrap().start_date;

        let left_max = monthly_playcount
            .iter()
            .map(|date_count| date_count.count)
            .max()
            .unwrap();

        let right_first = replays.first().unwrap().start_date;
        let right_last = replays.last().unwrap().start_date;

        let right_max = replays
            .iter()
            .map(|date_count| date_count.count)
            .max()
            .unwrap()
            .max(1);

        let right_label_area = match right_max {
            n if n < 10 => 40,
            n if n < 100 => 50,
            n if n < 1000 => 60,
            n if n < 10_000 => 70,
            n if n < 100_000 => 80,
            _ => 90,
        };

        let mut chart = ChartBuilder::on(&canvas)
            .margin(9)
            .x_label_area_size(20)
            .y_label_area_size(75)
            .right_y_label_area_size(right_label_area)
            .build_cartesian_2d((left_first..left_last).monthly(), 0..left_max)?
            .set_secondary_coord((right_first..right_last).monthly(), 0..right_max);

        // Mesh and labels
        chart
            .configure_mesh()
            .light_line_style(&BLACK.mix(0.0))
            .disable_x_mesh()
            .x_labels(10)
            .x_label_formatter(&|d| format!("{}-{}", d.year(), d.month()))
            .y_desc("Monthly playcount")
            .label_style(("sans-serif", 20))
            .draw()?;

        chart
            .configure_secondary_axes()
            .y_desc("Replays watched")
            .label_style(("sans-serif", 20))
            .draw()?;

        // Draw playcount area
        chart
            .draw_series(
                AreaSeries::new(
                    monthly_playcount
                        .iter()
                        .map(|MonthlyCount { start_date, count }| (*start_date, *count)),
                    0,
                    &BLUE.mix(0.2),
                )
                .border_style(&BLUE),
            )?
            .label("Monthly playcount")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE.stroke_width(2)));

        // Draw circles
        chart.draw_series(
            monthly_playcount
                .iter()
                .map(|MonthlyCount { start_date, count }| {
                    Circle::new((*start_date, *count), 2, BLUE.filled())
                }),
        )?;

        // Draw replay watched area
        chart
            .draw_secondary_series(
                AreaSeries::new(
                    replays
                        .iter()
                        .map(|MonthlyCount { start_date, count }| (*start_date, *count)),
                    0,
                    &RED.mix(0.2),
                )
                .border_style(&RED),
            )?
            .label("Replays watched")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED.stroke_width(2)));

        // Draw circles
        chart.draw_secondary_series(replays.iter().map(|MonthlyCount { start_date, count }| {
            Circle::new((*start_date, *count), 2, RED.filled())
        }))?;

        // Legend
        chart
            .configure_series_labels()
            .background_style(&RGBColor(192, 192, 192))
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(45)
            .label_font(("sans-serif", 20))
            .draw()?;
    }
    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.encode(&buf, W, H, ColorType::Rgb8)?;

    Ok(Some(png_bytes))
}
