use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    collections::HashMap,
    sync::Arc,
};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::{ListSize, MinimizedPp};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher, CowUtils,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{GameMode, Grade, OsuError, Score},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::{require_link, user_not_found, ScoreOrder, TopEntry};
use crate::{
    active::{impls::TopPagination, ActiveMessages},
    commands::GameModeOption,
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::{osu::UserArgs, RedisData},
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
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
async fn prefix_mapper(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(None, args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
pub async fn prefix_mappermania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Mania), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
pub async fn prefix_mappertaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Taiko), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
async fn prefix_mapperctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Catch), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a user's top100 are made by Sotarks?")]
#[usage("[username]")]
#[example("badewanne3")]
#[group(Osu)]
pub async fn prefix_sotarks(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match Mapper::args(Some(GameModeOption::Osu), args, Some("sotarks")) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_mapper(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Mapper::from_interaction(command.input_data())?;

    mapper(ctx, (&mut command).into(), args).await
}

async fn mapper(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Mapper<'_>) -> Result<()> {
    let msg_owner = orig.user_id()?;

    let mut config = match ctx.user_config().with_osu_id(msg_owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match config.osu.take() {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&ctx, &orig).await,
        },
    };

    let legacy_scores = match config.legacy_scores {
        Some(legacy_scores) => legacy_scores,
        None => match orig.guild_id() {
            Some(guild_id) => ctx
                .guild_config()
                .peek(guild_id, |config| config.legacy_scores)
                .await
                .unwrap_or(false),
            None => false,
        },
    };

    let mapper = args.mapper.cow_to_ascii_lowercase();
    let mapper_args = UserArgs::username(&ctx, mapper.as_ref()).await.mode(mode);
    let mapper_fut = ctx.redis().osu_user(mapper_args);

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);
    let scores_fut = ctx
        .osu_scores()
        .top(legacy_scores)
        .limit(100)
        .exec_with_user(user_args);

    let (mapper, user, scores) = match tokio::join!(mapper_fut, scores_fut) {
        (Ok(mapper), Ok((user, scores))) => (mapper, user, scores),
        (Err(OsuError::NotFound), _) => {
            let content = format!("Mapper with username `{mapper}` was not found");

            return orig.error(&ctx, content).await;
        }
        (_, Err(OsuError::NotFound)) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) | (_, Err(err)) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get mapper, user, or scores");

            return Err(err);
        }
    };

    let (mapper_name, mapper_id) = match &mapper {
        RedisData::Original(mapper) => (mapper.username.as_str(), mapper.user_id),
        RedisData::Archive(mapper) => (mapper.username.as_str(), mapper.user_id),
    };

    let username = user.username();

    let entries = match process_scores(&ctx, scores, mapper_id, args.sort).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

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
    let farm = HashMap::default();

    let list_size = match args.size.or(config.list_size) {
        Some(size) => size,
        None => match orig.guild_id() {
            Some(guild_id) => ctx
                .guild_config()
                .peek(guild_id, |config| config.list_size)
                .await
                .unwrap_or_default(),
            None => ListSize::default(),
        },
    };

    let minimized_pp = match config.minimized_pp {
        Some(minimized_pp) => minimized_pp,
        None => match list_size {
            ListSize::Condensed | ListSize::Detailed => MinimizedPp::default(),
            ListSize::Single => match orig.guild_id() {
                Some(guild_id) => ctx
                    .guild_config()
                    .peek(guild_id, |config| config.minimized_pp)
                    .await
                    .unwrap_or_default(),
                None => MinimizedPp::default(),
            },
        },
    };

    let pagination = TopPagination::builder()
        .user(user)
        .mode(mode)
        .entries(entries.into_boxed_slice())
        .sort_by(sort_by)
        .farm(farm)
        .list_size(list_size)
        .minimized_pp(minimized_pp)
        .content(content.into_boxed_str())
        .msg_owner(msg_owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, orig)
        .await
}

async fn process_scores(
    ctx: &Context,
    scores: Vec<Score>,
    mapper_id: u32,
    sort: Option<ScoreOrder>,
) -> Result<Vec<TopEntry>> {
    let mut entries = Vec::new();

    let maps_id_checksum = scores
        .iter()
        .filter_map(|score| score.map.as_ref())
        .filter(|map| map.creator_id == mapper_id)
        .map(|map| (map.map_id as i32, map.checksum.as_deref()))
        .collect();

    let mut maps = ctx.osu_map().maps(&maps_id_checksum).await?;

    for (i, score) in scores.into_iter().enumerate() {
        let Some(mut map) = maps.remove(&score.map_id) else {
            continue;
        };
        map.convert_mut(score.mode);

        let mut calc = ctx.pp(&map).mode(score.mode).mods(&score.mods);
        let attrs = calc.difficulty().await;
        let stars = attrs.stars() as f32;
        let max_combo = attrs.max_combo();

        let pp = score.pp.expect("missing pp");

        let max_pp = match score
            .pp
            .filter(|_| score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania)
        {
            Some(pp) => pp,
            None => calc.performance().await.pp() as f32,
        };

        let entry = TopEntry {
            original_idx: i,
            replay: score.replay,
            score: ScoreSlim::new(score, pp),
            map,
            max_pp,
            stars,
            max_combo,
        };

        entries.push(entry);
    }

    match sort {
        None => {}
        Some(ScoreOrder::Acc) => entries.sort_by(|a, b| {
            b.score
                .accuracy
                .partial_cmp(&a.score.accuracy)
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::Bpm) => entries.sort_by(|a, b| {
            b.map
                .bpm()
                .partial_cmp(&a.map.bpm())
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::Combo) => entries.sort_by_key(|entry| Reverse(entry.score.max_combo)),
        Some(ScoreOrder::Date) => entries.sort_by_key(|entry| Reverse(entry.score.ended_at)),
        Some(ScoreOrder::Length) => {
            entries.sort_by(|a, b| {
                let a_len = a.map.seconds_drain() as f32 / a.score.mods.clock_rate().unwrap_or(1.0);
                let b_len = b.map.seconds_drain() as f32 / b.score.mods.clock_rate().unwrap_or(1.0);

                b_len.partial_cmp(&a_len).unwrap_or(Ordering::Equal)
            });
        }
        Some(ScoreOrder::Misses) => entries.sort_by(|a, b| {
            b.score
                .statistics
                .count_miss
                .cmp(&a.score.statistics.count_miss)
                .then_with(|| {
                    let hits_a = a.score.total_hits();
                    let hits_b = b.score.total_hits();

                    let ratio_a = a.score.statistics.count_miss as f32 / hits_a as f32;
                    let ratio_b = b.score.statistics.count_miss as f32 / hits_b as f32;

                    ratio_b
                        .partial_cmp(&ratio_a)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
        }),
        Some(ScoreOrder::Pp) => entries.sort_by(|a, b| {
            b.score
                .pp
                .partial_cmp(&a.score.pp)
                .unwrap_or(Ordering::Equal)
        }),
        Some(ScoreOrder::RankedDate) => {
            entries.sort_by_key(|entry| Reverse(entry.map.ranked_date()))
        }
        Some(ScoreOrder::Score) => entries.sort_by_key(|entry| Reverse(entry.score.score)),
        Some(ScoreOrder::Stars) => {
            entries.sort_by(|a, b| b.stars.partial_cmp(&a.stars).unwrap_or(Ordering::Equal))
        }
    }

    Ok(entries)
}
