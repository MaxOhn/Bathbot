use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    fmt::Write,
};

use bathbot_macros::{HasMods, HasName, SlashCommand, command};
use bathbot_model::{
    PersonalBestIndex, command_fields::GameModeOption, embed_builder::SettingsImage,
};
use bathbot_psql::model::configs::{GuildConfig, ListSize, ScoreData};
use bathbot_util::{
    MessageOrigin,
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    osu::ModSelection,
    query::{IFilterCriteria, Searchable, TopCriteria},
};
use eyre::{Report, Result};
use rand::{Rng, thread_rng};
use rosu_v2::{
    prelude::{GameMode, OsuError, Score},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    guild::Permissions,
    id::{Id, marker::UserMarker},
};

use super::{HasMods, ModsResult, ScoreOrder, map_strains_graph, require_link, user_not_found};
use crate::{
    Context,
    active::{
        ActiveMessages,
        impls::{SingleScoreContent, SingleScorePagination, TopPagination},
    },
    commands::{
        DISCORD_OPTION_DESC, DISCORD_OPTION_HELP,
        utility::{
            MissAnalyzerCheck, ScoreEmbedDataHalf, ScoreEmbedDataPersonalBest, ScoreEmbedDataWrap,
        },
    },
    core::commands::{CommandOrigin, prefix::Args},
    manager::redis::osu::{UserArgs, UserArgsError, UserArgsSlim},
    util::{CheckPermissions, InteractionCommandExt, interaction::InteractionCommand},
};

#[derive(CommandModel, CreateCommand, HasMods, HasName, SlashCommand)]
#[command(name = "pinned", desc = "Display the user's pinned scores")]
pub struct Pinned<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<ScoreOrder>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    query: Option<Cow<'a, str>>,
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(desc = "Choose a specific score index or `random`")]
    index: Option<Cow<'a, str>>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
    #[command(
        desc = "Size of the embed",
        help = "Size of the embed.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
        The default can be set with the `/config` command."
    )]
    size: Option<ListSize>,
}

impl<'m> Pinned<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;
        let num = args.num;

        for arg in args {
            if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            mode,
            name,
            sort: None,
            query: None,
            reverse: None,
            mods: None,
            index: num.to_string_opt().map(Cow::Owned),
            discord,
            size: None,
        }
    }
}

#[command]
#[desc("Display the user's pinned scores")]
#[usage("[username]")]
#[examples("peppy")]
#[aliases("p", "pins")]
#[group(Osu)]
async fn prefix_pinned(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Pinned::args(None, args);

    pinned(msg.into(), args).await
}

#[command]
#[desc("Display the user's pinned taiko scores")]
#[usage("[username]")]
#[examples("peppy")]
#[aliases("ptaiko", "pinstaiko")]
#[group(Taiko)]
async fn prefix_pinnedtaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Pinned::args(Some(GameModeOption::Taiko), args);

    pinned(msg.into(), args).await
}

#[command]
#[desc("Display the user's pinned ctb scores")]
#[usage("[username]")]
#[examples("peppy")]
#[aliases("pcatch", "pctb", "pinnedcatch", "pinsctb", "pinscatch")]
#[group(Catch)]
async fn prefix_pinnedctb(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Pinned::args(Some(GameModeOption::Catch), args);

    pinned(msg.into(), args).await
}

#[command]
#[desc("Display the user's pinned mania scores")]
#[usage("[username]")]
#[examples("peppy")]
#[aliases("pmania", "pinsmania")]
#[group(Mania)]
async fn prefix_pinnedmania(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Pinned::args(Some(GameModeOption::Mania), args);

    pinned(msg.into(), args).await
}

async fn slash_pinned(mut command: InteractionCommand) -> Result<()> {
    let args = Pinned::from_interaction(command.input_data())?;

    pinned((&mut command).into(), args).await
}

async fn pinned(orig: CommandOrigin<'_>, args: Pinned<'_>) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(content).await;
        }
    };

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

    let guild_id = orig.guild_id();

    let GuildValues {
        list_size: guild_list_size,
        render_button: guild_render_button,
        score_data: guild_score_data,
    } = match guild_id {
        Some(guild_id) => {
            Context::guild_config()
                .peek(guild_id, |config| GuildValues::from(config))
                .await
        }
        None => GuildValues::default(),
    };

    let list_size = args
        .size
        .or(config.list_size)
        .or(guild_list_size)
        .unwrap_or_default();

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu.take() {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let (user_args, user_opt) = match UserArgs::rosu_id(&user_id, mode).await {
        UserArgs::Args(args) => (args, None),
        UserArgs::User { user, mode } => (
            UserArgsSlim::user_id(user.user_id.to_native()).mode(mode),
            Some(user),
        ),
        UserArgs::Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        UserArgs::Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let score_data = config.score_data.or(guild_score_data).unwrap_or_default();
    let legacy_scores = score_data.is_legacy();
    let missing_user = user_opt.is_none();

    let scores_manager = Context::osu_scores();
    let redis = Context::redis();
    let pinned_fut = scores_manager
        .clone()
        .pinned(legacy_scores)
        .limit(100)
        .exec(user_args);

    let top100_fut = async {
        if matches!(list_size, ListSize::Single) || args.index.is_some() {
            scores_manager
                .top(100, legacy_scores)
                .exec(user_args)
                .await
                .map(Some)
        } else {
            Ok(None)
        }
    };

    let user_fut = async {
        if missing_user {
            redis.osu_user_from_args(user_args).await.map(Some)
        } else {
            Ok(None)
        }
    };

    let (pinned_res, top100_res, user_res) = tokio::join!(pinned_fut, top100_fut, user_fut);

    let (pinned, top100, user) = match (pinned_res, top100_res, user_res) {
        (Ok(pinned), Ok(top100), Ok(user)) => {
            (pinned, top100, user.or(user_opt).expect("missing user"))
        }
        (Err(OsuError::NotFound), ..)
        | (_, Err(OsuError::NotFound), _)
        | (.., Err(UserArgsError::Osu(OsuError::NotFound))) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        (Err(err), ..) | (_, Err(err), _) | (.., Err(UserArgsError::Osu(err))) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user or prepare scores");

            return Err(err);
        }
        (.., Err(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

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

    let origin = MessageOrigin::new(guild_id, orig.channel_id());

    let pre_len = pinned.len();

    let entries = match process_scores(
        pinned,
        &args,
        mods.as_ref(),
        top100.as_deref(),
        with_render,
        legacy_scores,
        &origin,
    )
    .await
    {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to process scores"));
        }
    };

    let post_len = entries.len();
    let username = user.username.as_str();

    let index = match args.index.as_deref() {
        Some("random" | "?") => (post_len > 0).then(|| thread_rng().gen_range(1..=post_len)),
        Some(n) => match n.parse::<usize>() {
            Ok(n) if n > post_len => {
                let mut content = format!("`{username}` only has {post_len} pinned scores");

                if pre_len > post_len {
                    let _ = write!(content, " with the specified properties");
                }

                return orig.error(content).await;
            }
            Ok(n) => Some(n),
            Err(_) => {
                let content = "Failed to parse index. \
                Must be an integer between 1 and 200 or `random` / `?`.";

                return orig.error(content).await;
            }
        },
        None => None,
    };

    let single_idx = index
        .map(|num| num.saturating_sub(1))
        .or_else(|| (post_len == 1).then_some(0));

    let entries = entries.into_boxed_slice();

    let content = write_content(username, &args, entries.len(), mods.as_ref());
    let sort_by = args.sort.unwrap_or(ScoreOrder::Pp).into(); // TopOrder::Pp does not show anything

    let condensed_list = match (single_idx, list_size) {
        (Some(_), _) | (None, ListSize::Single) => {
            let content = content.map_or(SingleScoreContent::None, SingleScoreContent::SameForAll);

            let graph = match single_idx.map_or_else(|| entries.first(), |idx| entries.get(idx)) {
                Some(entry) if matches!(settings.image, SettingsImage::ImageWithStrains) => {
                    let entry = entry.get_half();

                    let fut = map_strains_graph(
                        &entry.map.pp_map,
                        entry.score.mods.clone(),
                        entry.map.cover(),
                        SingleScorePagination::IMAGE_W,
                        SingleScorePagination::IMAGE_H,
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

            let mut pagination = SingleScorePagination::new(
                &user, entries, settings, score_data, msg_owner, content,
            );

            if let Some(idx) = single_idx {
                pagination.set_index(idx);
            }

            return ActiveMessages::builder(pagination)
                .start_by_update(true)
                .attachment(graph)
                .begin(orig)
                .await;
        }
        (None, ListSize::Condensed) => true,
        (None, ListSize::Detailed) => false,
    };

    let pagination = TopPagination::builder()
        .user(user)
        .mode(mode)
        .entries(entries)
        .sort_by(sort_by)
        .condensed_list(condensed_list)
        .score_data(score_data)
        .content(content.unwrap_or_default().into_boxed_str())
        .msg_owner(msg_owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

async fn process_scores(
    pinned: Vec<Score>,
    args: &Pinned<'_>,
    mods: Option<&ModSelection>,
    top100: Option<&[Score]>,
    with_render: bool,
    legacy_scores: bool,
    origin: &MessageOrigin,
) -> Result<Vec<ScoreEmbedDataWrap>> {
    let filter_criteria = args.query.as_deref().map(TopCriteria::create);

    let mut entries = Vec::<ScoreEmbedDataWrap>::new();

    let maps_id_checksum = pinned
        .iter()
        .filter(|score| match mods {
            None => true,
            Some(selection) => selection.filter_score(score),
        })
        .filter_map(|score| score.map.as_ref())
        .map(|map| (map.map_id as i32, map.checksum.as_deref()))
        .collect();

    let maps = Context::osu_map().maps(&maps_id_checksum).await?;

    for (i, score) in pinned.into_iter().enumerate() {
        let Some(mut map) = maps.get(&score.map_id).cloned() else {
            continue;
        };

        map.convert_mut(score.mode);

        let mut half = ScoreEmbedDataHalf::new(
            score,
            map,
            None,
            legacy_scores,
            with_render,
            MissAnalyzerCheck::without(),
        )
        .await;

        half.pb_idx = top100.and_then(|top100| {
            let pb_idx =
                PersonalBestIndex::new(&half.score, half.map.map_id(), half.map.status(), top100);

            ScoreEmbedDataPersonalBest::try_new(pb_idx, origin)
        });

        half.original_idx = Some(i);

        if let Some(ref criteria) = filter_criteria {
            if half.matches(criteria) {
                entries.push(half.into());
            }
        } else {
            entries.push(half.into());
        }
    }

    match args.sort {
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

    if args.reverse.unwrap_or(false) {
        entries.reverse();
    }

    Ok(entries)
}

fn write_content(
    name: &str,
    args: &Pinned,
    amount: usize,
    mods: Option<&ModSelection>,
) -> Option<String> {
    if args.query.is_some() || mods.is_some() {
        Some(content_with_condition(args, amount, mods))
    } else if let Some(sort_by) = args.sort {
        let genitive = if name.ends_with('s') { "" } else { "s" };
        let as_reverse = args.reverse.unwrap_or(false);
        let reverse = if as_reverse { "reversed " } else { "" };

        let content = match sort_by {
            ScoreOrder::Acc => {
                format!("`{name}`'{genitive} pinned scores sorted by {reverse}accuracy:")
            }
            ScoreOrder::Bpm => format!("`{name}`'{genitive} pinned scores sorted by {reverse}BPM:"),
            ScoreOrder::Combo => {
                format!("`{name}`'{genitive} pinned scores sorted by {reverse}combo:")
            }
            ScoreOrder::Date if as_reverse => format!("Oldest pinned scores of `{name}`:"),
            ScoreOrder::Date => format!("Most recent pinned scores of `{name}`:"),
            ScoreOrder::Length => {
                format!("`{name}`'{genitive} pinned scores sorted by {reverse}length:")
            }
            ScoreOrder::Misses => {
                format!("`{name}`'{genitive} pinned scores sorted by {reverse}miss count:")
            }
            ScoreOrder::ModsCount => {
                format!("`{name}`'{genitive} pinned scores sorted by {reverse}amount of mods:")
            }
            ScoreOrder::Pp => format!("`{name}`'{genitive} pinned scores sorted by {reverse}pp"),
            ScoreOrder::RankedDate => {
                format!("`{name}`'{genitive} pinned scores sorted by {reverse}ranked date:")
            }
            ScoreOrder::Score => {
                format!("`{name}`'{genitive} pinned scores sorted by {reverse}score")
            }
            ScoreOrder::Stars => {
                format!("`{name}`'{genitive} pinned scores sorted by {reverse}stars")
            }
        };

        Some(content)
    } else if amount == 0 {
        Some(format!("`{name}` has not pinned any scores"))
    } else if amount == 1 {
        Some(format!("`{name}` has pinned 1 score:"))
    } else {
        None
    }
}

fn content_with_condition(args: &Pinned, amount: usize, mods: Option<&ModSelection>) -> String {
    let mut content = String::with_capacity(64);

    match args.sort {
        Some(ScoreOrder::Acc) => content.push_str("`Order: Accuracy"),
        Some(ScoreOrder::Bpm) => content.push_str("`Order: BPM"),
        Some(ScoreOrder::Combo) => content.push_str("`Order: Combo"),
        Some(ScoreOrder::Date) => content.push_str("`Order: Date"),
        Some(ScoreOrder::Length) => content.push_str("`Order: Length"),
        Some(ScoreOrder::Misses) => content.push_str("`Order: Miss count"),
        Some(ScoreOrder::ModsCount) => content.push_str("`Order: Amount of mods"),
        Some(ScoreOrder::Pp) => content.push_str("`Order: Pp"),
        Some(ScoreOrder::RankedDate) => content.push_str("`Order: Ranked date"),
        Some(ScoreOrder::Score) => content.push_str("`Order: Score"),
        Some(ScoreOrder::Stars) => content.push_str("`Order: Stars"),
        None => {}
    }

    if args.reverse.unwrap_or(false) {
        content.push_str(" (reverse)`");
    } else if !content.is_empty() {
        content.push('`');
    }

    if let Some(selection) = mods {
        if !content.is_empty() {
            content.push_str(" • ");
        }

        content.push_str("`Mods: ");

        let _ = match selection {
            ModSelection::Include(mods) => write!(content, "Include {mods}"),
            ModSelection::Exclude { mods, nomod: false } => write!(content, "Exclude {mods}"),
            ModSelection::Exclude { mods, nomod: true } => {
                write!(content, "Exclude NM (without {mods})")
            }
            ModSelection::Exact(mods) => write!(content, "{mods}"),
        };

        content.push('`');
    }

    if let Some(query) = args.query.as_deref() {
        TopCriteria::create(query).display(&mut content);
    }

    let plural = if amount == 1 { "" } else { "s" };
    let _ = write!(content, "\nFound {amount} matching pinned score{plural}:");

    content
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
