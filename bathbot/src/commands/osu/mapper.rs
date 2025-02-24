use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
};

use bathbot_macros::{HasName, SlashCommand, command};
use bathbot_model::{command_fields::GameModeOption, embed_builder::SettingsImage};
use bathbot_psql::model::configs::{GuildConfig, ListSize, ScoreData};
use bathbot_util::{CowUtils, constants::GENERAL_ISSUE, matcher};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameMode, OsuError, Score},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    guild::Permissions,
    id::{Id, marker::UserMarker},
};

use super::{ScoreOrder, map_strain_graph, require_link, user_not_found};
use crate::{
    Context,
    active::{
        ActiveMessages,
        impls::{SingleScoreContent, SingleScorePagination, TopPagination},
    },
    commands::utility::{MissAnalyzerCheck, ScoreEmbedDataPersonalBest, ScoreEmbedDataWrap},
    core::commands::{CommandOrigin, prefix::Args},
    manager::redis::osu::{UserArgs, UserArgsError},
    util::{ChannelExt, CheckPermissions, InteractionCommandExt, interaction::InteractionCommand},
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "mapper",
    desc = "How often does the given mapper appear in top a user's top plays",
    help = "Count the top plays on maps of the given mapper.\n\
    It will try to consider guest difficulties so that if a map was created by someone else \
    but the given mapper made the guest diff, it will count.\n\
    Similarly, if the given mapper created the mapset but someone else guest diff'd, \
    it will not count.\n\
    This does not always work perfectly, especially for older maps but it's what the api provides."
)]
pub struct Mapper<'a> {
    #[command(desc = "Specify a mapper username")]
    mapper: Cow<'a, str>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<ScoreOrder>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(
        desc = "Size of the embed",
        help = "Size of the embed.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
        The default can be set with the `/config` command."
    )]
    size: Option<ListSize>,
}

impl<'m> Mapper<'m> {
    fn args(
        mode: Option<GameModeOption>,
        mut args: Args<'m>,
        mapper: Option<&'static str>,
    ) -> Result<Self, &'static str> {
        let mapper = match mapper.or_else(|| args.next()) {
            Some(arg) => arg.into(),
            None => {
                let content = "You need to specify at least one osu! username for the mapper. \
                    If you're not linked, you must specify at least two names.";

                return Err(content);
            }
        };

        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Ok(Self {
            mapper,
            mode,
            name,
            sort: None,
            discord,
            size: None,
        })
    }
}

#[command]
#[desc("How many maps of a user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a user which were mapped by the given mapper.\n\
    Specify the __mapper first__ and the __user second__."
)]
#[usage("[mapper] [user]")]
#[example("\"Hishiro Chizuru\" badewanne3", "monstrata monstrata")]
#[group(Osu)]
async fn prefix_mapper(msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(None, args, None) {
        Ok(args) => mapper(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a mania user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a mania user which were mapped by the given mapper.\n\
    Specify the __mapper first__ and the __user second__."
)]
#[usage("[mapper] [user]")]
#[example("\"Hishiro Chizuru\" badewanne3", "monstrata monstrata")]
#[alias("mapperm")]
#[group(Mania)]
pub async fn prefix_mappermania(msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Mania), args, None) {
        Ok(args) => mapper(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a taiko user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a taiko user which were mapped by the given mapper.\n\
    Specify the __mapper first__ and the __user second__."
)]
#[usage("[mapper] [user]")]
#[example("\"Hishiro Chizuru\" badewanne3", "monstrata monstrata")]
#[alias("mappert")]
#[group(Taiko)]
pub async fn prefix_mappertaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Taiko), args, None) {
        Ok(args) => mapper(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a ctb user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a ctb user which were mapped by the given mapper.\n\
    Specify the __mapper first__ and the __user second__."
)]
#[usage("[mapper] [user]")]
#[example("\"Hishiro Chizuru\" badewanne3", "monstrata monstrata")]
#[aliases("mapperc", "mappercatch")]
#[group(Catch)]
async fn prefix_mapperctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Catch), args, None) {
        Ok(args) => mapper(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a user's top100 are made by Sotarks?")]
#[usage("[username]")]
#[example("badewanne3")]
#[group(Osu)]
pub async fn prefix_sotarks(msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Osu), args, Some("sotarks")) {
        Ok(args) => mapper(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

async fn slash_mapper(mut command: InteractionCommand) -> Result<()> {
    let args = Mapper::from_interaction(command.input_data())?;

    mapper((&mut command).into(), args).await
}

async fn mapper(orig: CommandOrigin<'_>, args: Mapper<'_>) -> Result<()> {
    let msg_owner = orig.user_id()?;

    let mut config = match Context::user_config().with_osu_id(msg_owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu.take() {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let GuildValues {
        list_size: guild_list_size,
        render_button: guild_render_button,
        score_data: guild_score_data,
    } = match orig.guild_id() {
        Some(guild_id) => {
            Context::guild_config()
                .peek(guild_id, |config| GuildValues::from(config))
                .await
        }
        None => GuildValues::default(),
    };

    let score_data = config.score_data.or(guild_score_data).unwrap_or_default();
    let legacy_scores = score_data.is_legacy();

    let mapper = args.mapper.cow_to_ascii_lowercase();
    let mapper_args = UserArgs::username(mapper.as_ref(), mode).await;
    let mapper_fut = Context::redis().osu_user(mapper_args);

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&user_id, mode).await;
    let scores_fut = Context::osu_scores()
        .top(legacy_scores)
        .limit(100)
        .exec_with_user(user_args);

    let (mapper, user, scores) = match tokio::join!(mapper_fut, scores_fut) {
        (Ok(mapper), Ok((user, scores))) => (mapper, user, scores),
        (Err(UserArgsError::Osu(OsuError::NotFound)), _) => {
            let content = format!("Mapper with username `{mapper}` was not found");

            return orig.error(content).await;
        }
        (_, Err(UserArgsError::Osu(OsuError::NotFound))) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get mapper, user, or scores");

            return Err(err);
        }
    };

    let mapper_name = mapper.username.as_str();
    let mapper_id = mapper.user_id.to_native();

    let username = user.username.as_str();
    let settings = config.score_embed.unwrap_or_default();

    let mut with_render = match (guild_render_button, config.render_button) {
        (None | Some(true), None) => true,
        (None | Some(true), Some(with_render)) => with_render,
        (Some(false), _) => false,
    };

    with_render &= settings.buttons.render
        && mode == GameMode::Osu
        && orig.has_permission_to(Permissions::SEND_MESSAGES)
        && Context::ordr_available();

    let entries =
        match process_scores(scores, mapper_id, args.sort, with_render, legacy_scores).await {
            Ok(entries) => entries,
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to process scores"));
            }
        };

    // Accumulate all necessary data
    let content = match mapper_name {
        "Sotarks" => {
            let amount = entries.len();

            let mut content = format!(
                "I found {amount} Sotarks map{plural} in `{username}`'s top100, ",
                amount = amount,
                plural = if amount != 1 { "s" } else { "" },
            );

            let to_push = match amount {
                0 => "I'm proud \\:)",
                1..=4 => "that's already too many...",
                5..=8 => "kinda sad \\:/",
                9..=15 => "pretty sad \\:(",
                16..=25 => "this is so sad \\:((",
                26..=35 => "this needs to stop",
                36..=49 => "that's a serious problem...",
                50 => "that's half. HALF.",
                51..=79 => "how do you sleep at night...",
                80..=99 => "i'm not even mad, that's just impressive",
                100 => "you did it. \"Congrats\".",
                _ => "wait how did you do that",
            };

            content.push_str(to_push);

            content
        }
        _ => format!(
            "{count} of `{username}`'{genitive} top score maps were mapped by `{mapper_name}`",
            count = entries.len(),
            genitive = if username.ends_with('s') { "" } else { "s" },
        ),
    };

    let sort_by = args.sort.unwrap_or(ScoreOrder::Pp).into();

    let list_size = args
        .size
        .or(config.list_size)
        .or(guild_list_size)
        .unwrap_or_default();

    let entries = entries.into_boxed_slice();

    let condensed_list = match list_size {
        ListSize::Condensed => true,
        ListSize::Detailed => false,
        ListSize::Single => {
            let content = SingleScoreContent::SameForAll(content);

            let graph = match entries.first() {
                Some(entry) if matches!(settings.image, SettingsImage::ImageWithStrains) => {
                    let entry = entry.get_half();

                    let fut = map_strain_graph(
                        &entry.map.pp_map,
                        entry.score.mods.clone(),
                        entry.map.cover(),
                    );

                    match fut.await {
                        Ok(graph) => Some((SingleScorePagination::IMAGE_NAME.to_owned(), graph)),
                        Err(err) => {
                            warn!(?err, "Failed to create strain graph");

                            None
                        }
                    }
                }
                Some(_) | None => None,
            };

            let pagination = SingleScorePagination::new(
                &user, entries, settings, score_data, msg_owner, content,
            );

            return ActiveMessages::builder(pagination)
                .start_by_update(true)
                .attachment(graph)
                .begin(orig)
                .await;
        }
    };

    let pagination = TopPagination::builder()
        .user(user)
        .mode(mode)
        .entries(entries)
        .sort_by(sort_by)
        .condensed_list(condensed_list)
        .score_data(score_data)
        .content(content.into_boxed_str())
        .msg_owner(msg_owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

async fn process_scores(
    scores: Vec<Score>,
    mapper_id: u32,
    sort: Option<ScoreOrder>,
    with_render: bool,
    legacy_scores: bool,
) -> Result<Vec<ScoreEmbedDataWrap>> {
    let mut entries = Vec::new();

    let maps_id_checksum = scores
        .iter()
        .filter_map(|score| score.map.as_ref())
        .filter(|map| map.creator_id == mapper_id)
        .map(|map| (map.map_id as i32, map.checksum.as_deref()))
        .collect();

    let mut maps = Context::osu_map().maps(&maps_id_checksum).await?;

    for (i, score) in scores.into_iter().enumerate() {
        let Some(mut map) = maps.remove(&score.map_id) else {
            continue;
        };

        map.convert_mut(score.mode);

        let pb_idx = Some(ScoreEmbedDataPersonalBest::from_index(i));

        let entry = ScoreEmbedDataWrap::new_half(
            score,
            map,
            pb_idx,
            legacy_scores,
            with_render,
            MissAnalyzerCheck::without(),
        )
        .await;

        entries.push(entry);
    }

    match sort {
        None => {}
        Some(ScoreOrder::Acc) => entries.sort_by(|a, b| {
            b.get_half()
                .score
                .accuracy
                .partial_cmp(&a.get_half().score.accuracy)
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::Bpm) => entries.sort_by(|a, b| {
            b.get_half()
                .map
                .bpm()
                .partial_cmp(&a.get_half().map.bpm())
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::Combo) => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().score.max_combo))
        }
        Some(ScoreOrder::Date) => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().score.ended_at))
        }
        Some(ScoreOrder::Length) => {
            entries.sort_by(|a, b| {
                let a_len = a.get_half().map.seconds_drain() as f64
                    / a.get_half().score.mods.clock_rate().unwrap_or(1.0);
                let b_len = b.get_half().map.seconds_drain() as f64
                    / b.get_half().score.mods.clock_rate().unwrap_or(1.0);

                b_len.partial_cmp(&a_len).unwrap_or(Ordering::Equal)
            });
        }
        Some(ScoreOrder::Misses) => entries.sort_by(|a, b| {
            let a = a.get_half();
            let b = b.get_half();

            b.score
                .statistics
                .miss
                .cmp(&a.score.statistics.miss)
                .then_with(|| {
                    let hits_a = a.score.total_hits();
                    let hits_b = b.score.total_hits();

                    let ratio_a = a.score.statistics.miss as f32 / hits_a as f32;
                    let ratio_b = b.score.statistics.miss as f32 / hits_b as f32;

                    ratio_b
                        .partial_cmp(&ratio_a)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
        }),
        Some(ScoreOrder::ModsCount) => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().score.mods.len()))
        }
        Some(ScoreOrder::Pp) => entries.sort_by(|a, b| {
            b.get_half()
                .score
                .pp
                .partial_cmp(&a.get_half().score.pp)
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::RankedDate) => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().map.ranked_date()))
        }
        Some(ScoreOrder::Score) => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().score.score))
        }
        Some(ScoreOrder::Stars) => entries.sort_by(|a, b| {
            b.get_half()
                .stars
                .partial_cmp(&a.get_half().stars)
                .unwrap_or(Ordering::Equal)
        }),
    }

    Ok(entries)
}

#[derive(Default)]
struct GuildValues {
    list_size: Option<ListSize>,
    render_button: Option<bool>,
    score_data: Option<ScoreData>,
}

impl From<&GuildConfig> for GuildValues {
    fn from(config: &GuildConfig) -> Self {
        Self {
            list_size: config.list_size,
            render_button: config.render_button,
            score_data: config.score_data,
        }
    }
}
